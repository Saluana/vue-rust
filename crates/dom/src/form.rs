//! Form controls (WP-U0): input value and checkedness, radio groups, textarea, select and option selectedness, and the
//! button type. Value sanitization covers text, url, email, number, range and color; the other input types keep the
//! value as typed (date, time and file are not modelled). Selectedness follows Chrome: the option that becomes
//! selected last wins, and an empty single select selects its first enabled option.

use crate::attr::Change;
use crate::error::DomError;
use crate::node::{NodeId, NodeKind, Namespace};
use crate::value::{number_to_string, strip_collapse};
use crate::Document;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    /// The value is the dirty value once set, else the value attribute sanitized (text-like types, number, range, color).
    Value,
    /// The value is the value attribute itself (hidden, submit, image, reset, button).
    Default,
    /// The value is the value attribute, or "on" (checkbox, radio).
    DefaultOn,
    /// File inputs have no value to set here.
    File,
}

const INPUT_TYPES: &[&str] = &[
    "hidden", "text", "search", "tel", "url", "email", "password", "date", "month", "week", "time", "datetime-local",
    "number", "range", "color", "checkbox", "radio", "file", "submit", "image", "reset", "button",
];

/// A JavaScript-style floating-point literal (no leading plus, no whitespace).
fn valid_float(s: &str) -> Option<f64> {
    let body = s.strip_prefix('-').unwrap_or(s);
    let b = body.as_bytes();
    let mut i = 0;
    let mut digits = 0;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
        digits += 1;
    }
    if i < b.len() && b[i] == b'.' {
        i += 1;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
            digits += 1;
        }
    }
    if digits == 0 {
        return None;
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        i += 1;
        if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
            i += 1;
        }
        let start = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == start {
            return None;
        }
    }
    (i == b.len()).then(|| s.parse::<f64>().ok()).flatten()
}

fn normalize_newlines(s: &str) -> String {
    s.replace("\r\n", "\n").replace('\r', "\n")
}

/// Value sanitization for the value mode of input types. `min` and `max` are the raw attribute values (range only).
fn sanitize(ty: &str, raw: &str, min: Option<&str>, max: Option<&str>) -> String {
    match ty {
        "url" | "email" => raw.replace(['\r', '\n'], "").trim_matches(|c| matches!(c, ' ' | '\t' | '\u{c}')).to_string(),
        "number" => match valid_float(raw) {
            Some(_) => raw.to_string(),
            None => String::new(),
        },
        "range" => {
            let lo = min.and_then(valid_float).unwrap_or(0.0);
            let hi = max.and_then(valid_float).unwrap_or(100.0).max(lo);
            let default = lo + (hi - lo) / 2.0;
            let v = valid_float(raw).unwrap_or(default).clamp(lo, hi);
            number_to_string(v)
        }
        "color" => {
            let hex = raw.strip_prefix('#').unwrap_or("");
            if raw.len() == 7 && hex.bytes().all(|c| c.is_ascii_hexdigit()) {
                raw.to_ascii_lowercase()
            } else {
                "#000000".to_string()
            }
        }
        "text" | "search" | "tel" | "password" => raw.replace(['\r', '\n'], ""),
        _ => raw.to_string(),
    }
}

impl Document {
    pub(crate) fn is_html_element_named(&self, id: NodeId, tag: &str) -> bool {
        let n = self.n(id);
        n.kind == NodeKind::Element && n.ns == Namespace::Html && n.local == tag
    }

    pub(crate) fn is_html_input(&self, id: NodeId) -> bool {
        self.is_html_element_named(id, "input")
    }

    pub(crate) fn is_html_label(&self, id: NodeId) -> bool {
        self.is_html_element_named(id, "label")
    }

    pub(crate) fn is_html_option(&self, id: NodeId) -> bool {
        self.is_html_element_named(id, "option")
    }

    /// Descendants in tree order (not including `root`).
    pub(crate) fn descendants(&self, root: NodeId) -> Vec<NodeId> {
        let mut out = Vec::new();
        let mut stack: Vec<NodeId> = self.child_nodes(root).into_iter().rev().collect();
        while let Some(x) = stack.pop() {
            out.push(x);
            stack.extend(self.child_nodes(x).into_iter().rev());
        }
        out
    }

    /// The normalised type of an input (lowercase, unknown values are text); None for other nodes.
    pub(crate) fn input_type_name(&self, id: NodeId) -> Option<&'static str> {
        if !self.is_html_input(id) {
            return None;
        }
        let lower = self.attr_raw(id, "type").unwrap_or_default().to_ascii_lowercase();
        Some(INPUT_TYPES.iter().find(|t| **t == lower).copied().unwrap_or("text"))
    }

    pub(crate) fn input_mode(&self, id: NodeId) -> Mode {
        match self.input_type_name(id) {
            Some("hidden" | "submit" | "image" | "reset" | "button") => Mode::Default,
            Some("checkbox" | "radio") => Mode::DefaultOn,
            Some("file") => Mode::File,
            _ => Mode::Value,
        }
    }

    /// The IDL `value` of an input (the empty string for other nodes).
    pub(crate) fn input_value(&self, id: NodeId) -> String {
        if !self.is_html_input(id) {
            return String::new();
        }
        let attr = self.attr_raw(id, "value");
        match self.input_mode(id) {
            Mode::Value => {
                let form = &self.n(id).form;
                if form.value_dirty {
                    form.value.clone()
                } else {
                    let raw = attr.unwrap_or_default();
                    let min = self.attr_raw(id, "min");
                    let max = self.attr_raw(id, "max");
                    sanitize(self.input_type_name(id).unwrap_or("text"), &raw, min.as_deref(), max.as_deref())
                }
            }
            Mode::Default => attr.unwrap_or_default(),
            Mode::DefaultOn => attr.unwrap_or_else(|| "on".to_string()),
            Mode::File => String::new(),
        }
    }

    /// The IDL `value` setter for an input. Errors: NotSupportedError for file inputs (no file selection here).
    pub(crate) fn set_input_value(&mut self, id: NodeId, value: &str) -> Result<(), DomError> {
        match self.input_mode(id) {
            Mode::Value => {
                let min = self.attr_raw(id, "min");
                let max = self.attr_raw(id, "max");
                let ty = self.input_type_name(id).unwrap_or("text");
                let stored = sanitize(ty, value, min.as_deref(), max.as_deref());
                let form = &mut self.nm(id).form;
                form.value = stored;
                form.value_dirty = true;
                Ok(())
            }
            Mode::Default | Mode::DefaultOn => {
                self.write_attr(id, "value", value);
                Ok(())
            }
            Mode::File => Err(DomError::NotSupportedError),
        }
    }

    pub(crate) fn is_disabled_control(&self, id: NodeId) -> bool {
        ["button", "input", "select", "textarea"].iter().any(|t| self.is_html_element_named(id, t))
            && self.attr_raw(id, "disabled").is_some()
    }

    fn is_labelable(&self, id: NodeId) -> bool {
        self.is_html_element_named(id, "button")
            || self.is_html_element_named(id, "select")
            || self.is_html_element_named(id, "textarea")
            || (self.is_html_input(id) && self.input_type_name(id) != Some("hidden"))
    }

    /// The labeled control: the labelable element whose id is the `for` attribute, else the first labelable descendant.
    pub(crate) fn label_control(&self, label: NodeId) -> Option<NodeId> {
        match self.attr_raw(label, "for") {
            Some(target) => {
                let root = self.root_of(label);
                self.descendants(root).into_iter().find(|&x| {
                    self.attr_raw(x, "id").as_deref() == Some(target.as_str()) && self.is_labelable(x)
                })
            }
            None => self.descendants(label).into_iter().find(|&x| self.is_labelable(x)),
        }
    }

    fn is_radio(&self, id: NodeId) -> bool {
        self.input_type_name(id) == Some("radio")
    }

    /// The other radio buttons in the same group: same non-empty name, same tree.
    fn radio_group_others(&self, id: NodeId) -> Vec<NodeId> {
        if !self.is_radio(id) {
            return Vec::new();
        }
        let name = self.attr_raw(id, "name").unwrap_or_default();
        if name.is_empty() {
            return Vec::new();
        }
        let root = self.root_of(id);
        self.descendants(root)
            .into_iter()
            .filter(|&x| x != id && self.is_radio(x) && self.attr_raw(x, "name").as_deref() == Some(name.as_str()))
            .collect()
    }

    pub(crate) fn checked_radio_in_group(&self, id: NodeId) -> Option<NodeId> {
        self.radio_group_others(id).into_iter().find(|&x| self.n(x).form.checked)
    }

    /// checkedness without the dirty flag: a checked radio unchecks its group.
    fn set_checked_internal(&mut self, id: NodeId, value: bool) {
        self.nm(id).form.checked = value;
        if value && self.is_radio(id) {
            for x in self.radio_group_others(id) {
                self.nm(x).form.checked = false;
            }
        }
    }

    /// The IDL `checked` setter: also sets the dirty checkedness flag.
    pub(crate) fn set_checked(&mut self, id: NodeId, value: bool) {
        self.nm(id).form.checked_dirty = true;
        self.set_checked_internal(id, value);
    }

    /// Adding or removing the `checked` attribute changes checkedness only while the element is not dirty.
    pub(crate) fn checked_attribute_changed(&mut self, id: NodeId, change: Change) {
        if !self.is_html_input(id) || self.n(id).form.checked_dirty {
            return;
        }
        match change {
            Change::Added => self.set_checked_internal(id, true),
            Change::Removed => self.nm(id).form.checked = false,
            Change::Replaced => {}
        }
    }

    /// A radio that gets a name while checked takes its new group.
    pub(crate) fn radio_name_changed(&mut self, id: NodeId) {
        if self.is_radio(id) && self.n(id).form.checked {
            self.set_checked_internal(id, true);
        }
    }

    /// A `type` change: a value that was carried by value mode moves into the value attribute when the input leaves
    /// value mode; a checked radio takes its group.
    pub(crate) fn input_type_changed(&mut self, id: NodeId, old_mode: Mode, old_value: String) {
        if !self.is_html_input(id) {
            return;
        }
        let new_mode = self.input_mode(id);
        // Chrome writes the value into the value attribute only when there is a value to carry over (an empty
        // input that becomes a checkbox gets no value attribute).
        if old_mode == Mode::Value && matches!(new_mode, Mode::Default | Mode::DefaultOn) && !old_value.is_empty() {
            self.write_attr(id, "value", &old_value);
        }
        if self.is_radio(id) && self.n(id).form.checked {
            self.set_checked_internal(id, true);
        }
    }

    /// The button type: submit, reset or button (case-insensitive); anything else is submit.
    pub(crate) fn button_type(&self, id: NodeId) -> &'static str {
        match self.attr_raw(id, "type").unwrap_or_default().to_ascii_lowercase().as_str() {
            "reset" => "reset",
            "button" => "button",
            _ => "submit",
        }
    }

    pub(crate) fn textarea_value(&self, id: NodeId) -> String {
        let form = &self.n(id).form;
        if form.value_dirty {
            form.value.clone()
        } else {
            normalize_newlines(&self.text_content(id).unwrap_or_default())
        }
    }

    pub(crate) fn set_textarea_value(&mut self, id: NodeId, value: &str) {
        let form = &mut self.nm(id).form;
        form.value = normalize_newlines(value);
        form.value_dirty = true;
    }

    // ---- select and option -------------------------------------------------------------------------------------

    pub(crate) fn options_of(&self, select: NodeId) -> Vec<NodeId> {
        self.descendants(select).into_iter().filter(|&x| self.is_html_option(x)).collect()
    }

    /// select.options: the option elements below a select, in tree order (a snapshot of the live list).
    pub fn option_ids(&self, select: NodeId) -> Vec<NodeId> {
        self.options_of(select)
    }

    /// The select that owns an option: its parent, or the parent of its optgroup.
    pub(crate) fn select_of_option(&self, opt: NodeId) -> Option<NodeId> {
        let p = self.n(opt).parent?;
        if self.is_html_element_named(p, "select") {
            return Some(p);
        }
        if self.is_html_element_named(p, "optgroup") {
            let gp = self.n(p).parent?;
            if self.is_html_element_named(gp, "select") {
                return Some(gp);
            }
        }
        None
    }

    /// The select whose options are affected by a change to `p`: p itself, or the select above an optgroup.
    fn select_context(&self, p: NodeId) -> Option<NodeId> {
        if self.is_html_element_named(p, "select") {
            Some(p)
        } else if self.is_html_element_named(p, "optgroup") {
            self.n(p).parent.filter(|&gp| self.is_html_element_named(gp, "select"))
        } else {
            None
        }
    }

    fn select_is_multiple(&self, select: NodeId) -> bool {
        self.attr_raw(select, "multiple").is_some()
    }

    fn option_is_disabled(&self, opt: NodeId) -> bool {
        self.attr_raw(opt, "disabled").is_some()
    }

    /// Single selects always have a selected option when they have one enabled option.
    fn select_reset(&mut self, select: NodeId) {
        if self.select_is_multiple(select) {
            return;
        }
        let opts = self.options_of(select);
        if opts.iter().any(|&o| self.n(o).form.selected) {
            return;
        }
        if let Some(&first) = opts.iter().find(|&&o| !self.option_is_disabled(o)) {
            self.nm(first).form.selected = true;
        }
    }

    /// A single select keeps only its last selected option (tree order).
    fn select_enforce_single(&mut self, select: NodeId) {
        if self.select_is_multiple(select) {
            return;
        }
        let selected: Vec<NodeId> = self.options_of(select).into_iter().filter(|&o| self.n(o).form.selected).collect();
        if selected.len() > 1 {
            for &o in &selected[..selected.len() - 1] {
                self.nm(o).form.selected = false;
            }
        }
    }

    /// An option became selected: in a single select every other option is deselected.
    fn option_became_selected(&mut self, opt: NodeId) {
        let Some(select) = self.select_of_option(opt) else { return };
        if self.select_is_multiple(select) {
            return;
        }
        for o in self.options_of(select) {
            if o != opt {
                self.nm(o).form.selected = false;
            }
        }
    }

    /// Inserting a subtree: a checked radio takes its group (the last one in tree order wins), and a selected option
    /// wins over its select's selection; the empty-select reset applies afterwards.
    pub(crate) fn after_insert(&mut self, child: NodeId) {
        let mut subtree = vec![child];
        subtree.extend(self.descendants(child));
        for x in subtree {
            if self.is_html_option(x) && self.n(x).form.selected {
                self.option_became_selected(x);
            }
            if self.is_radio(x) && self.n(x).form.checked {
                self.set_checked_internal(x, true);
            }
        }
    }

    /// Removing or inserting children of `p` can empty a select's selection.
    pub(crate) fn after_tree_change(&mut self, parents: &[Option<NodeId>]) {
        for p in parents.iter().flatten() {
            if let Some(select) = self.select_context(*p) {
                self.select_reset(select);
            }
        }
    }

    /// Option `selected` attribute added (Added) or removed, while not dirty.
    pub(crate) fn selected_attribute_changed(&mut self, id: NodeId, change: Change) {
        if !self.is_html_option(id) || self.n(id).form.selected_dirty {
            return;
        }
        match change {
            Change::Added => {
                self.nm(id).form.selected = true;
                self.option_became_selected(id);
            }
            Change::Removed => {
                self.nm(id).form.selected = false;
                if let Some(select) = self.select_of_option(id) {
                    self.select_reset(select);
                }
            }
            Change::Replaced => {}
        }
    }

    /// The IDL `selected` setter: dirty; selecting deselects the others, deselecting resets an empty select.
    pub(crate) fn set_option_selected(&mut self, opt: NodeId, value: bool) {
        let form = &mut self.nm(opt).form;
        form.selected = value;
        form.selected_dirty = true;
        if value {
            self.option_became_selected(opt);
        } else if let Some(select) = self.select_of_option(opt) {
            self.select_reset(select);
        }
    }

    pub(crate) fn option_disabled_changed(&mut self, id: NodeId) {
        if self.is_html_option(id) {
            if let Some(select) = self.select_of_option(id) {
                self.select_reset(select);
            }
        }
    }

    pub(crate) fn multiple_attribute_changed(&mut self, id: NodeId) {
        if self.is_html_element_named(id, "select") {
            self.select_enforce_single(id);
            self.select_reset(id);
        }
    }

    /// selectedIndex: the first selected option's index, or -1.
    pub(crate) fn select_index(&self, select: NodeId) -> i32 {
        self.options_of(select).iter().position(|&o| self.n(o).form.selected).map_or(-1, |i| i as i32)
    }

    /// selectedIndex setter: only the option at `index` is selected (dirty); an out-of-range index selects nothing.
    pub(crate) fn set_select_index(&mut self, select: NodeId, index: i32) {
        let opts = self.options_of(select);
        for &o in &opts {
            self.nm(o).form.selected = false;
        }
        if let Some(&o) = usize::try_from(index).ok().and_then(|i| opts.get(i)) {
            let form = &mut self.nm(o).form;
            form.selected = true;
            form.selected_dirty = true;
        }
    }

    /// select.value: the value of the first selected option, or "".
    pub(crate) fn select_value(&self, select: NodeId) -> String {
        self.options_of(select)
            .into_iter()
            .find(|&o| self.n(o).form.selected)
            .map(|o| self.option_value(o))
            .unwrap_or_default()
    }

    /// select.value setter: the first option whose value matches is selected (dirty); no match selects nothing.
    pub(crate) fn set_select_value(&mut self, select: NodeId, value: &str) {
        let opts = self.options_of(select);
        for &o in &opts {
            self.nm(o).form.selected = false;
        }
        if let Some(&hit) = opts.iter().find(|&&o| self.option_value(o) == value) {
            let form = &mut self.nm(hit).form;
            form.selected = true;
            form.selected_dirty = true;
        }
    }

    /// option.value: the value attribute, else the text.
    pub(crate) fn option_value(&self, opt: NodeId) -> String {
        match self.attr_raw(opt, "value") {
            Some(v) => v,
            None => self.option_text(opt),
        }
    }

    /// option.text: the textContent with ASCII whitespace stripped and collapsed.
    pub(crate) fn option_text(&self, opt: NodeId) -> String {
        strip_collapse(&self.text_content(opt).unwrap_or_default())
    }
}
