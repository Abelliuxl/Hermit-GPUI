use gpui::prelude::FluentBuilder;
use gpui::{
    actions, div, fill, hsla, point, px, relative, rgba, size, App, Bounds, Context, CursorStyle,
    Element, ElementId, ElementInputHandler, Entity, EntityInputHandler, EventEmitter, FocusHandle,
    Focusable, GlobalElementId, InspectorElementId, InteractiveElement, IntoElement, KeyBinding,
    LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, ParentElement,
    Pixels, Point, Render, SharedString, Size, StatefulInteractiveElement, Style, Styled, TextRun,
    UTF16Selection, Window, WrappedLine,
};
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

actions!(
    editor,
    [
        Backspace,
        Delete,
        Left,
        Right,
        Up,
        Down,
        SelectLeft,
        SelectRight,
        SelectUp,
        SelectDown,
        SelectAll,
        Home,
        End,
        ShowCharacterPalette,
        Paste,
        Cut,
        Copy,
        Newline,
        Submit,
    ]
);

pub fn bind_editor_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, Some("Editor")),
        KeyBinding::new("delete", Delete, Some("Editor")),
        KeyBinding::new("left", Left, Some("Editor")),
        KeyBinding::new("right", Right, Some("Editor")),
        KeyBinding::new("up", Up, Some("Editor")),
        KeyBinding::new("down", Down, Some("Editor")),
        KeyBinding::new("shift-left", SelectLeft, Some("Editor")),
        KeyBinding::new("shift-right", SelectRight, Some("Editor")),
        KeyBinding::new("shift-up", SelectUp, Some("Editor")),
        KeyBinding::new("shift-down", SelectDown, Some("Editor")),
        KeyBinding::new("cmd-a", SelectAll, Some("Editor")),
        KeyBinding::new("home", Home, Some("Editor")),
        KeyBinding::new("end", End, Some("Editor")),
        KeyBinding::new("cmd-v", Paste, Some("Editor")),
        KeyBinding::new("cmd-c", Copy, Some("Editor")),
        KeyBinding::new("cmd-x", Cut, Some("Editor")),
        KeyBinding::new("ctrl-cmd-space", ShowCharacterPalette, Some("Editor")),
        KeyBinding::new("shift-enter", Newline, Some("Editor")),
        KeyBinding::new("enter", Submit, Some("Editor")),
    ]);
}

#[derive(Clone, Debug, PartialEq)]
pub enum EditorEvent {
    /// Enter pressed (composer send / field commit).
    Submit,
    /// Content changed.
    Change,
}

struct LineEntry {
    /// Byte offset of this logical line inside the content.
    byte_start: usize,
    line: WrappedLine,
}

pub struct Editor {
    focus_handle: FocusHandle,
    content: String,
    placeholder: SharedString,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    lines: Vec<LineEntry>,
    last_bounds: Option<Bounds<Pixels>>,
    is_selecting: bool,
    max_rows: f32,
    /// Single-line fields hide newlines and use input cursor style.
    single_line: bool,
}

impl Editor {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self::new_with(cx, false)
    }

    pub fn single_line(cx: &mut Context<Self>) -> Self {
        Self::new_with(cx, true)
    }

    fn new_with(cx: &mut Context<Self>, single_line: bool) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            content: String::new(),
            placeholder: "Type here…".into(),
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            lines: Vec::new(),
            last_bounds: None,
            is_selecting: false,
            max_rows: if single_line { 1.0 } else { 8.0 },
            single_line,
        }
    }

    pub fn set_placeholder(
        &mut self,
        placeholder: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
        self.placeholder = placeholder.into();
        cx.notify();
    }

    pub fn text(&self) -> &str {
        &self.content
    }

    pub fn set_text(&mut self, text: impl Into<String>, cx: &mut Context<Self>) {
        self.content = text.into();
        self.selected_range = self.content.len()..self.content.len();
        self.selection_reversed = false;
        self.marked_range = None;
        cx.notify();
    }

    pub fn clear(&mut self, cx: &mut Context<Self>) {
        self.set_text("", cx);
    }

    pub fn focus(&self, window: &mut Window) {
        window.focus(&self.focus_handle);
    }

    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    // -- movement ---------------------------------------------------------

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.selected_range = offset..offset;
        self.selection_reversed = false;
        cx.notify();
    }

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        if self.selection_reversed {
            self.selected_range.start = offset;
        } else {
            self.selected_range.end = offset;
        }
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        cx.notify();
    }

    fn clamp(&self, offset: usize) -> usize {
        offset.min(self.content.len())
    }

    /// Start-of-line (or start-of-text for Home) index.
    fn line_start(&self, offset: usize) -> usize {
        self.content[..self.clamp(offset)]
            .rfind('\n')
            .map(|index| index + 1)
            .unwrap_or(0)
    }

    /// End-of-line index (exclusive of the newline itself).
    fn line_end(&self, offset: usize) -> usize {
        let offset = self.clamp(offset);
        self.content[offset..]
            .find('\n')
            .map(|index| offset + index)
            .unwrap_or(self.content.len())
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .rev()
            .find_map(|(index, _)| (index < offset).then_some(index))
            .unwrap_or(0)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .find_map(|(index, _)| (index > offset).then_some(index))
            .unwrap_or(self.content.len())
    }

    // -- utf16 mapping (IME) ----------------------------------------------

    fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf8_offset = 0;
        let mut utf16_count = 0;
        for ch in self.content.chars() {
            if utf16_count >= offset {
                break;
            }
            utf16_count += ch.len_utf16();
            utf8_offset += ch.len_utf8();
        }
        utf8_offset
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        let mut utf16_offset = 0;
        let mut utf8_count = 0;
        for ch in self.content.chars() {
            if utf8_count >= offset {
                break;
            }
            utf8_count += ch.len_utf8();
            utf16_offset += ch.len_utf16();
        }
        utf16_offset
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    fn range_from_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range.start)..self.offset_from_utf16(range.end)
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        if self.content.is_empty() {
            return 0;
        }
        let Some(bounds) = self.last_bounds.as_ref() else {
            return 0;
        };
        let line_height = px(ROW_HEIGHT);
        for (row_index, entry) in self.lines.iter().enumerate() {
            let rows = entry.line.wrap_boundaries().len() + 1;
            let top = bounds.top() + px(self.rows_above(row_index) as f32 * ROW_HEIGHT);
            let span = px(ROW_HEIGHT * rows as f32);
            if position.y >= top && position.y <= top + span {
                let local = point(position.x - bounds.left(), position.y - top);
                return entry.byte_start
                    + entry
                        .line
                        .closest_index_for_position(local, line_height)
                        .unwrap_or_else(|closest| closest);
            }
        }
        // Below/above all lines: clamp to start or end.
        if position.y < bounds.top() {
            0
        } else {
            self.content.len()
        }
    }

    fn rows_above(&self, up_to: usize) -> usize {
        self.lines[..up_to.min(self.lines.len())]
            .iter()
            .map(|entry| entry.line.wrap_boundaries().len() + 1)
            .sum()
    }

    // -- editing -----------------------------------------------------------

    fn replace_text_in_range_internal(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range| self.range_from_utf16(range))
            .or(self.marked_range.clone())
            .unwrap_or_else(|| self.selected_range.clone());
        self.content = format!(
            "{}{new_text}{}",
            &self.content[..range.start],
            &self.content[range.end..]
        );
        self.selected_range = range.start + new_text.len()..range.start + new_text.len();
        self.marked_range = None;
        cx.emit(EditorEvent::Change);
        cx.notify();
    }

    fn insert_newline(&mut self, cx: &mut Context<Self>) {
        if self.single_line {
            cx.emit(EditorEvent::Submit);
            return;
        }
        self.replace_text_in_range_internal(None, "\n", cx);
    }

    fn backspace(&mut self, _: &Backspace, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            let offset = self.cursor_offset();
            self.select_to(self.previous_boundary(offset), cx);
        }
        self.replace_text_in_range_internal(None, "", cx);
    }

    fn delete(&mut self, _: &Delete, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            let offset = self.cursor_offset();
            self.select_to(self.next_boundary(offset), cx);
        }
        self.replace_text_in_range_internal(None, "", cx);
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.previous_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.start, cx);
        }
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.next_boundary(self.selected_range.end), cx);
        } else {
            self.move_to(self.selected_range.end, cx);
        }
    }

    fn up(&mut self, _: &Up, window: &mut Window, cx: &mut Context<Self>) {
        let cursor = self.cursor_offset();
        let line_start = self.line_start(cursor);
        if line_start == 0 {
            self.move_to(0, cx);
            return;
        }
        // Column in bytes within the current line; find same column on the
        // previous line via the pixel position of the cursor.
        let column = cursor - line_start;
        let prev_line_start = self.line_start(line_start - 1);
        let prev_line_len = line_start - 1 - prev_line_start;
        let target = prev_line_start + column.min(prev_line_len);
        self.move_to(target, cx);
        let _ = window;
    }

    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        let cursor = self.cursor_offset();
        let line_end = self.line_end(cursor);
        if line_end >= self.content.len() {
            self.move_to(self.content.len(), cx);
            return;
        }
        let line_start = self.line_start(cursor);
        let column = cursor - line_start;
        let next_line_start = line_end + 1;
        let next_line_end = self.line_end(next_line_start);
        let target = next_line_start + column.min(next_line_end - next_line_start);
        self.move_to(target, cx);
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.line_start(self.cursor_offset()), cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.line_end(self.cursor_offset()), cx);
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor_offset()), cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor_offset()), cx);
    }

    fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        let cursor = self.cursor_offset();
        let line_start = self.line_start(cursor);
        if line_start == 0 {
            self.select_to(0, cx);
            return;
        }
        let column = cursor - line_start;
        let prev_line_start = self.line_start(line_start - 1);
        let prev_len = line_start - 1 - prev_line_start;
        self.select_to(prev_line_start + column.min(prev_len), cx);
    }

    fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        let cursor = self.cursor_offset();
        let line_end = self.line_end(cursor);
        if line_end >= self.content.len() {
            self.select_to(self.content.len(), cx);
            return;
        }
        let line_start = self.line_start(cursor);
        let column = cursor - line_start;
        let next_start = line_end + 1;
        let next_end = self.line_end(next_start);
        self.select_to(next_start + column.min(next_end - next_start), cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.selected_range = 0..self.content.len();
        cx.notify();
    }

    fn paste(&mut self, _: &Paste, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.replace_text_in_range_internal(None, &text, cx);
        }
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
        }
    }

    fn cut(&mut self, _: &Cut, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
            self.replace_text_in_range_internal(None, "", cx);
        }
    }

    fn show_character_palette(
        &mut self,
        _: &ShowCharacterPalette,
        window: &mut Window,
        _: &mut Context<Self>,
    ) {
        window.show_character_palette();
    }

    fn submit(&mut self, _: &Submit, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(EditorEvent::Submit);
    }

    // -- mouse ---------------------------------------------------------------

    fn on_mouse_down(&mut self, event: &MouseDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.is_selecting = true;
        if event.modifiers.shift {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        } else {
            self.move_to(self.index_for_mouse_position(event.position), cx);
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        }
    }

    fn estimated_rows(&self) -> usize {
        if self.single_line {
            return 1;
        }
        let logical_lines = self.content.split('\n').count().max(1);
        // Rough wrap estimate: ~90 columns per row.
        let wraps: usize = self
            .content
            .split('\n')
            .map(|line| line.len().saturating_sub(1) / 90)
            .sum();
        (logical_lines + wraps).max(1)
    }
}

impl Focusable for Editor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EventEmitter<EditorEvent> for Editor {}

pub const ROW_HEIGHT: f32 = 20.0;

impl EntityInputHandler for Editor {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.content[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected_range),
            reversed: self.selection_reversed,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.marked_range = None;
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.replace_text_in_range_internal(range_utf16, new_text, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range| self.range_from_utf16(range))
            .or(self.marked_range.clone())
            .unwrap_or_else(|| self.selected_range.clone());
        self.content = format!(
            "{}{new_text}{}",
            &self.content[..range.start],
            &self.content[range.end..]
        );
        if !new_text.is_empty() {
            self.marked_range = Some(range.start..range.start + new_text.len());
        } else {
            self.marked_range = None;
        }
        self.selected_range = new_selected_range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .map(|new_range| new_range.start + range.start..new_range.end + range.end)
            .unwrap_or_else(|| range.start + new_text.len()..range.start + new_text.len());
        cx.emit(EditorEvent::Change);
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let range = self.range_from_utf16(&range_utf16);
        let row = self.row_for_byte(range.start)?;
        let entry = &self.lines[row];
        let local = entry
            .line
            .position_for_index(range.start - entry.byte_start, px(ROW_HEIGHT))?;
        let top = bounds.top() + px(self.rows_above(row) as f32 * ROW_HEIGHT);
        Some(Bounds::new(
            point(bounds.left() + local.x, top),
            size(px(2.0), px(ROW_HEIGHT)),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        Some(self.index_for_mouse_position(point))
    }
}

impl Editor {
    fn row_for_byte(&self, byte: usize) -> Option<usize> {
        self.lines
            .iter()
            .rposition(|entry| entry.byte_start <= byte)
    }
}

/// The painted element: shapes + paints the wrapped text, cursor and selection.
struct EditorElement {
    entity: Entity<Editor>,
}

struct PrepaintState {
    lines: Vec<LineEntry>,
    content_size: Size<Pixels>,
    cursor: Option<PaintQuad>,
    selection: Vec<PaintQuad>,
}

impl IntoElement for EditorElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for EditorElement {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let editor = self.entity.read(cx);
        let rows = editor.estimated_rows().min(8).max(1);
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = px(ROW_HEIGHT * rows as f32).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let editor = self.entity.read(cx);
        let content = editor.content.clone();
        let selected_range = editor.selected_range.clone();
        let cursor = editor.cursor_offset();
        let style = window.text_style();
        let font_size = style.font_size.to_pixels(window.rem_size());

        let run = TextRun {
            len: content.len(),
            font: style.font(),
            color: style.color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };

        let wrapped = if content.is_empty() {
            let placeholder_run = TextRun {
                len: editor.placeholder.len(),
                font: style.font(),
                color: hsla(0., 0., 0.5, 0.45),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            window
                .text_system()
                .shape_text(
                    editor.placeholder.clone(),
                    font_size,
                    &[placeholder_run],
                    Some(bounds.size.width),
                    None,
                )
                .unwrap_or_default()
        } else {
            window
                .text_system()
                .shape_text(
                    content.clone().into(),
                    font_size,
                    &[run],
                    Some(bounds.size.width),
                    None,
                )
                .unwrap_or_default()
        };

        // Byte offsets for each logical line.
        let mut lines: Vec<LineEntry> = Vec::with_capacity(wrapped.len());
        let mut byte_start = 0usize;
        for line in wrapped {
            lines.push(LineEntry { byte_start, line });
            // Advance past this line's text plus the newline that follows it.
            let text_len = {
                let remainder = &content[byte_start.min(content.len())..];
                match remainder.find('\n') {
                    Some(newline) => newline,
                    None => remainder.len(),
                }
            };
            byte_start += text_len + 1;
        }

        // Cursor + selection.
        let mut cursor_quad = None;
        let mut selection_quads = Vec::new();
        let is_focused = self.entity.read(cx).focus_handle.is_focused(window);
        if is_focused {
            let row_for = |byte: usize, lines: &[LineEntry]| -> Option<usize> {
                lines.iter().rposition(|entry| entry.byte_start <= byte)
            };
            if selected_range.is_empty() {
                if let Some(row) = row_for(cursor, &lines) {
                    let entry = &lines[row];
                    let rows_above: usize = lines[..row]
                        .iter()
                        .map(|entry| entry.line.wrap_boundaries().len() + 1)
                        .sum();
                    if let Some(local) = entry
                        .line
                        .position_for_index(cursor - entry.byte_start, px(ROW_HEIGHT))
                    {
                        let y = bounds.top() + px(rows_above as f32 * ROW_HEIGHT) + local.y;
                        cursor_quad = Some(fill(
                            Bounds::new(
                                point(bounds.left() + local.x, y),
                                size(px(2.0), px(ROW_HEIGHT)),
                            ),
                            gpui::blue(),
                        ));
                    }
                }
            } else {
                let start = selected_range.start.min(selected_range.end);
                let end = selected_range.start.max(selected_range.end);
                for (row, entry) in lines.iter().enumerate() {
                    let line_end = entry.byte_start + entry.line.len();
                    let intersect_start = start.max(entry.byte_start);
                    let intersect_end = end.min(line_end);
                    if intersect_start >= intersect_end {
                        continue;
                    }
                    let rows_above: usize = lines[..row]
                        .iter()
                        .map(|entry| entry.line.wrap_boundaries().len() + 1)
                        .sum();
                    let start_local = entry
                        .line
                        .position_for_index(intersect_start - entry.byte_start, px(ROW_HEIGHT));
                    let end_local = entry
                        .line
                        .position_for_index(intersect_end - entry.byte_start, px(ROW_HEIGHT));
                    if let (Some(start_local), Some(end_local)) = (start_local, end_local) {
                        // One quad per wrapped row is overkill for v1: draw the
                        // whole span on one row when it fits, else a full-row
                        // highlight.
                        if (start_local.y - end_local.y).abs() < px(0.5) {
                            let y =
                                bounds.top() + px(rows_above as f32 * ROW_HEIGHT) + start_local.y;
                            selection_quads.push(fill(
                                Bounds::new(
                                    point(bounds.left() + start_local.x, y),
                                    size(
                                        (end_local.x - start_local.x).max(px(2.0)),
                                        px(ROW_HEIGHT),
                                    ),
                                ),
                                rgba(0x3311ff30),
                            ));
                        } else {
                            let rows = entry.line.wrap_boundaries().len() + 1;
                            let y = bounds.top() + px(rows_above as f32 * ROW_HEIGHT);
                            selection_quads.push(fill(
                                Bounds::new(
                                    point(bounds.left(), y),
                                    size(bounds.size.width, px(ROW_HEIGHT * rows as f32)),
                                ),
                                rgba(0x3311ff30),
                            ));
                        }
                    }
                }
            }
        }

        let content_rows: usize = lines
            .iter()
            .map(|entry| entry.line.wrap_boundaries().len() + 1)
            .sum();
        let content_size = size(
            bounds.size.width,
            px(ROW_HEIGHT * content_rows.max(1) as f32),
        );

        PrepaintState {
            lines,
            content_size,
            cursor: cursor_quad,
            selection: selection_quads,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.entity.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.entity.clone()),
            cx,
        );

        for quad in prepaint.selection.drain(..) {
            window.paint_quad(quad);
        }

        let line_height = px(ROW_HEIGHT);
        let mut rows_above = 0usize;
        for entry in &prepaint.lines {
            let origin = point(
                bounds.left(),
                bounds.top() + px(rows_above as f32 * ROW_HEIGHT),
            );
            let _ = entry
                .line
                .paint(origin, line_height, gpui::TextAlign::Left, None, window, cx);
            rows_above += entry.line.wrap_boundaries().len() + 1;
        }

        if focus_handle.is_focused(window) {
            if let Some(cursor) = prepaint.cursor.take() {
                window.paint_quad(cursor);
            }
        }

        let lines = std::mem::take(&mut prepaint.lines);
        let content_size = prepaint.content_size;
        self.entity.update(cx, |editor, _cx| {
            editor.lines = lines;
            editor.last_bounds = Some(bounds);
        });
        let _ = content_size;
    }
}

impl Render for Editor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let single_line = self.single_line;
        div()
            .id("editor-hitbox")
            .flex()
            .key_context("Editor")
            .track_focus(&self.focus_handle(cx))
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::down))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_up))
            .on_action(cx.listener(Self::select_down))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::show_character_palette))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::newline))
            .on_action(cx.listener(Self::submit))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .text_size(gpui::px(13.0))
            .line_height(gpui::px(ROW_HEIGHT))
            .child(
                div()
                    .id("editor-element")
                    .w_full()
                    .when(single_line, |this| {
                        this.overflow_x_scroll().whitespace_nowrap()
                    })
                    .child(EditorElement { entity }),
            )
    }
}

impl Editor {
    fn newline(&mut self, _: &Newline, _: &mut Window, cx: &mut Context<Self>) {
        self.insert_newline(cx);
    }
}
