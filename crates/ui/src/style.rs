//! ComputedStyle: one typed value per longhand of the table (indexed like `props::PROPS`), the custom properties of
//! the element, and the Chrome-style `get` accessor used by the oracle comparisons.

use crate::num;
use crate::props;
use crate::serialize;
use crate::value::Value;
use vn_css::custom::CustomProperties;

#[derive(Debug, Clone)]
pub struct ComputedStyle {
    values: Vec<Value>,
    custom: CustomProperties,
    /// The computed font size in px (line-height numbers serialise as px of this).
    font_size: f64,
}

impl ComputedStyle {
    pub(crate) fn new(values: Vec<Value>, custom: CustomProperties, font_size: f64) -> ComputedStyle {
        ComputedStyle { values, custom, font_size }
    }

    /// The computed value of a longhand (None for a name outside the table).
    pub fn value(&self, name: &str) -> Option<&Value> {
        props::index_of(name).map(|i| &self.values[i])
    }

    pub fn custom(&self) -> &CustomProperties {
        &self.custom
    }

    /// The computed value at a table index (crate-internal: inheritance reads the parent by index).
    pub(crate) fn at(&self, idx: usize) -> &Value {
        &self.values[idx]
    }

    /// The computed font size in px.
    pub fn font_size(&self) -> f64 {
        self.font_size
    }

    /// The Chrome serialisation of a longhand or a custom property (`--name`), as getPropertyValue returns it. Custom
    /// properties that are absent give an empty string, as Chrome does. Shorthands are not served (None).
    pub fn get(&self, name: &str) -> Option<String> {
        if name.starts_with("--") {
            return Some(self.custom.get(name).map_or_else(String::new, |t| t.text().to_string()));
        }
        let v = self.value(name)?;
        Some(match (name, v) {
            ("line-height", Value::Num(n)) => format!("{}px", num::fmt(n * self.font_size)),
            // getComputedStyle resolves `auto` minimum sizes to 0px for a box that is not a flex or grid item.
            ("min-width" | "min-height", Value::Kw("auto")) => "0px".to_string(),
            _ => serialize::value_text(v),
        })
    }
}
