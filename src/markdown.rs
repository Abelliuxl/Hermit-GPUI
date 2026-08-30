/// Markdown block parser, a direct port of the SwiftUI MarkdownBlockParser:
/// headings, paragraphs, bullets, numbered lists, quotes, fenced code, tables
/// and separators. Inline markup is rendered by the view layer.
#[derive(Clone, Debug, PartialEq)]
pub enum MarkdownBlock {
    Heading(i32, String),
    Paragraph(String),
    Bullet(String),
    Numbered(i64, String),
    Quote(String),
    Code(String),
    Table(Vec<String>, Vec<Vec<String>>),
    Separator,
}

pub fn parse(content: &str) -> Vec<MarkdownBlock> {
    let normalized = normalize(content);
    let lines: Vec<&str> = normalized.lines().collect();
    let mut blocks: Vec<MarkdownBlock> = Vec::new();
    let mut paragraph: Vec<String> = Vec::new();
    let mut code_lines: Vec<String> = Vec::new();
    let mut in_code_block = false;

    fn flush_paragraph(blocks: &mut Vec<MarkdownBlock>, paragraph: &mut Vec<String>) {
        let text = paragraph
            .iter()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_string();
        if !text.is_empty() {
            blocks.push(MarkdownBlock::Paragraph(text));
        }
        paragraph.clear();
    }

    let mut index = 0usize;
    while index < lines.len() {
        let raw_line = lines[index];
        let line = raw_line.trim();

        if line.starts_with("```") {
            if in_code_block {
                blocks.push(MarkdownBlock::Code(code_lines.join("\n")));
                code_lines.clear();
                in_code_block = false;
            } else {
                flush_paragraph(&mut blocks, &mut paragraph);
                in_code_block = true;
            }
            index += 1;
            continue;
        }

        if in_code_block {
            code_lines.push(raw_line.to_string());
            index += 1;
            continue;
        }

        if line.is_empty() {
            flush_paragraph(&mut blocks, &mut paragraph);
            index += 1;
            continue;
        }

        if is_table_start(&lines, index) {
            flush_paragraph(&mut blocks, &mut paragraph);
            let headers = split_table_line(lines[index]);
            index += 2;
            let mut rows: Vec<Vec<String>> = Vec::new();
            while index < lines.len() {
                let candidate = lines[index].trim();
                if !is_table_line(candidate) || is_table_separator(candidate) {
                    break;
                }
                rows.push(split_table_line(candidate));
                index += 1;
            }
            blocks.push(MarkdownBlock::Table(headers, rows));
            continue;
        }

        if line.len() >= 3 && line.chars().all(|c| c == '-' || c == '*' || c == '_') {
            flush_paragraph(&mut blocks, &mut paragraph);
            blocks.push(MarkdownBlock::Separator);
            index += 1;
            continue;
        }

        if let Some(captures) = match_regex(line, r"^(#{1,6})\s+(.+)$") {
            if captures.len() == 2 {
                flush_paragraph(&mut blocks, &mut paragraph);
                let level = captures[0].chars().filter(|c| *c == '#').count() as i32;
                blocks.push(MarkdownBlock::Heading(level, captures[1].clone()));
                index += 1;
                continue;
            }
        }

        if let Some(captures) = match_regex(line, r"^(\d{1,3})[.)]\s+(.+)$") {
            if captures.len() == 2 {
                if let Ok(number) = captures[0].parse::<i64>() {
                    flush_paragraph(&mut blocks, &mut paragraph);
                    blocks.push(MarkdownBlock::Numbered(number, captures[1].clone()));
                    index += 1;
                    continue;
                }
            }
        }

        if let Some(captures) = match_regex(line, r"^(\d{1,3})([A-Za-z_][A-Za-z0-9_.\-].*)$") {
            if captures.len() == 2 {
                if let Ok(number) = captures[0].parse::<i64>() {
                    flush_paragraph(&mut blocks, &mut paragraph);
                    blocks.push(MarkdownBlock::Numbered(number, captures[1].clone()));
                    index += 1;
                    continue;
                }
            }
        }

        if let Some(captures) = match_regex(line, r"^[-*•]\s+(.+)$") {
            if !captures.is_empty() {
                flush_paragraph(&mut blocks, &mut paragraph);
                blocks.push(MarkdownBlock::Bullet(captures[0].clone()));
                index += 1;
                continue;
            }
        }

        if let Some(captures) = match_regex(line, r"^>\s?(.+)$") {
            if !captures.is_empty() {
                flush_paragraph(&mut blocks, &mut paragraph);
                blocks.push(MarkdownBlock::Quote(captures[0].clone()));
                index += 1;
                continue;
            }
        }

        paragraph.push(raw_line.to_string());
        index += 1;
    }

    if in_code_block {
        blocks.push(MarkdownBlock::Code(code_lines.join("\n")));
    }
    flush_paragraph(&mut blocks, &mut paragraph);

    if blocks.is_empty() {
        blocks.push(MarkdownBlock::Paragraph(normalized));
    }
    blocks
}

/// Does the content contain a markdown table (used to pick streaming font)?
pub fn contains_markdown_table(content: &str) -> bool {
    let lines: Vec<&str> = content.split('\n').collect();
    if lines.len() < 2 {
        return false;
    }
    for index in 0..lines.len() - 1 {
        if lines[index].contains('|') && is_table_separator(lines[index + 1]) {
            return true;
        }
    }
    false
}

fn normalize(content: &str) -> String {
    let mut text = content.replace("\r\n", "\n").replace('\r', "\n");

    if !text.contains('\n') && text.contains("\\n") {
        text = text.replace("\\n", "\n");
    }

    if !text.contains('\n') {
        if let Ok(re) =
            regex::Regex::new(r"(?<![A-Za-z0-9.])([1-9][0-9]?)([A-Za-z_][A-Za-z0-9_.\-]+)")
        {
            text = re
                .replace_all(&text, "\n$1. $2")
                .trim_matches(|c: char| c.is_whitespace())
                .to_string();
            return text;
        }
    }

    text.trim().to_string()
}

fn match_regex(text: &str, pattern: &str) -> Option<Vec<String>> {
    let re = regex::Regex::new(pattern).ok()?;
    let captures = re.captures(text)?;
    let mut out = Vec::new();
    for group in captures.iter().skip(1) {
        out.push(group?.as_str().to_string());
    }
    Some(out)
}

fn is_table_start(lines: &[&str], index: usize) -> bool {
    if index + 1 >= lines.len() {
        return false;
    }
    is_table_line(lines[index]) && is_table_separator(lines[index + 1])
}

fn is_table_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.contains('|') && split_table_line(trimmed).len() >= 2
}

fn is_table_separator(line: &str) -> bool {
    let cells = split_table_line(line);
    if cells.len() < 2 {
        return false;
    }
    cells.iter().all(|cell| {
        let value = cell.trim();
        if value.is_empty() || !value.contains('-') {
            return false;
        }
        value.chars().all(|c| c == '-' || c == ':')
    })
}

fn split_table_line(line: &str) -> Vec<String> {
    let mut trimmed = line.trim();
    if trimmed.starts_with('|') {
        trimmed = &trimmed[1..];
    }
    if trimmed.ends_with('|') && !trimmed.is_empty() {
        trimmed = &trimmed[..trimmed.len() - 1];
    }
    trimmed
        .split('|')
        .map(|cell| cell.trim().to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_headings_bullets_code() {
        let blocks = parse("# Title\n\n- item one\n- item two\n\n```rust\nfn a() {}\n```");
        assert_eq!(
            blocks,
            vec![
                MarkdownBlock::Heading(1, "Title".into()),
                MarkdownBlock::Bullet("item one".into()),
                MarkdownBlock::Bullet("item two".into()),
                MarkdownBlock::Code("fn a() {}".into()),
            ]
        );
    }

    #[test]
    fn parses_table() {
        let blocks = parse("| a | b |\n| --- | --- |\n| 1 | 2 |\n| 3 | 4 |");
        match &blocks[0] {
            MarkdownBlock::Table(headers, rows) => {
                assert_eq!(headers, &vec!["a".to_string(), "b".to_string()]);
                assert_eq!(rows.len(), 2);
            }
            other => panic!("expected table, got {other:?}"),
        }
    }

    #[test]
    fn numbered_list_and_quote() {
        let blocks = parse("1. first\n2. second\n> quoted");
        assert_eq!(
            blocks,
            vec![
                MarkdownBlock::Numbered(1, "first".into()),
                MarkdownBlock::Numbered(2, "second".into()),
                MarkdownBlock::Quote("quoted".into()),
            ]
        );
    }

    #[test]
    fn compact_numbered_only_in_single_line_text() {
        // single-line: "3Step one" becomes a numbered item
        let blocks = parse("3Step one");
        assert_eq!(blocks[0], MarkdownBlock::Numbered(3, "Step one".into()));
        // multi-line keeps "12.4 GB" intact
        let blocks = parse("Disk shows 12.4 GB free.\nAll good.");
        assert!(matches!(blocks[0], MarkdownBlock::Paragraph(_)));
    }
}
