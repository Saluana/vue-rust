//! Reflecting IDL properties (WP-U0). `has_property` answers like `name in element` for the names listed here and
//! false for every other name. Not modelled, and answered false although Chrome has them: innerText, dataset, lang,
//! dir, accessKey, classList, attributes, contentEditable, isContentEditable, labels, form, list, width, height, size,
//! maxLength, pattern, options' length, onclick and every other event handler, and all methods. `get_property` of an
//! unmodelled name returns undefined; `set_property` of one is UnsupportedProperty (JavaScript would create an expando).
//! Object-valued properties (style, children, childNodes, options, parentNode, firstChild) answer `has` but
//! `get_property` is UnsupportedProperty: a HostValue cannot hold an object.

use crate::error::DomError;
use crate::events::Invoke;
use crate::node::{NodeId, NodeKind, Namespace};
use crate::value::{HostValue, number_to_string};
use crate::Document;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Prop {
    NodeName,
    NodeType,
    ParentNode,
    FirstChild,
    ChildNodes,
    IsConnected,
    TextContent,
    NodeValue,
    LocalName,
    TagName,
    NamespaceUri,
    Children,
    Style,
    InnerHtml,
    OuterHtml,
    Id,
    ClassName,
    TabIndex,
    Title,
    Hidden,
    Value,
    DefaultValue,
    Checked,
    DefaultChecked,
    Selected,
    DefaultSelected,
    SelectedIndex,
    Options,
    Text,
    Type,
    Name,
    Placeholder,
    ReadOnly,
    Required,
    Multiple,
    Min,
    Max,
    Step,
    Href,
    Src,
    Alt,
    HtmlFor,
    Disabled,
}

impl Prop {
    /// The content attribute behind a string-valued reflecting property.
    fn string_attr(self) -> Option<&'static str> {
        Some(match self {
            Prop::Id => "id",
            Prop::ClassName => "class",
            Prop::Title => "title",
            Prop::Name => "name",
            Prop::Placeholder => "placeholder",
            Prop::Min => "min",
            Prop::Max => "max",
            Prop::Step => "step",
            Prop::Href => "href",
            Prop::Src => "src",
            Prop::Alt => "alt",
            Prop::HtmlFor => "for",
            _ => return None,
        })
    }

    /// The content attribute behind a boolean reflecting property.
    fn bool_attr(self) -> Option<&'static str> {
        Some(match self {
            Prop::Hidden => "hidden",
            Prop::Disabled => "disabled",
            Prop::ReadOnly => "readonly",
            Prop::Required => "required",
            Prop::Multiple => "multiple",
            Prop::DefaultChecked => "checked",
            Prop::DefaultSelected => "selected",
            _ => return None,
        })
    }
}

impl Document {
    /// The supported property `name` of node `id`, if any.
    pub(crate) fn prop_of(&self, id: NodeId, name: &str) -> Option<Prop> {
        let node = match name {
            "nodeName" => Prop::NodeName,
            "nodeType" => Prop::NodeType,
            "parentNode" => Prop::ParentNode,
            "firstChild" => Prop::FirstChild,
            "childNodes" => Prop::ChildNodes,
            "isConnected" => Prop::IsConnected,
            "textContent" => Prop::TextContent,
            "nodeValue" => Prop::NodeValue,
            _ => {
                return match self.n(id).kind {
                    NodeKind::Document if name == "children" => Some(Prop::Children),
                    NodeKind::Element => self.element_prop(id, name),
                    _ => None,
                };
            }
        };
        Some(node)
    }

    fn element_prop(&self, id: NodeId, name: &str) -> Option<Prop> {
        let common = match name {
            "localName" => Some(Prop::LocalName),
            "tagName" => Some(Prop::TagName),
            "namespaceURI" => Some(Prop::NamespaceUri),
            "children" => Some(Prop::Children),
            "style" => Some(Prop::Style),
            "innerHTML" => Some(Prop::InnerHtml),
            "outerHTML" => Some(Prop::OuterHtml),
            "id" => Some(Prop::Id),
            "className" => Some(Prop::ClassName),
            "tabIndex" => Some(Prop::TabIndex),
            _ => None,
        };
        if common.is_some() {
            return common;
        }
        let n = self.n(id);
        if n.ns != Namespace::Html {
            return None;
        }
        let tag = n.local.as_str();
        match name {
            "title" => Some(Prop::Title),
            "hidden" => Some(Prop::Hidden),
            _ => self.html_tag_prop(tag, name),
        }
    }

    fn html_tag_prop(&self, tag: &str, name: &str) -> Option<Prop> {
        let allowed: &[(&str, Prop)] = match tag {
            "input" => &[
                ("value", Prop::Value), ("checked", Prop::Checked), ("defaultChecked", Prop::DefaultChecked),
                ("defaultValue", Prop::DefaultValue), ("type", Prop::Type), ("name", Prop::Name),
                ("placeholder", Prop::Placeholder), ("readOnly", Prop::ReadOnly), ("required", Prop::Required),
                ("multiple", Prop::Multiple), ("min", Prop::Min), ("max", Prop::Max), ("step", Prop::Step),
                ("disabled", Prop::Disabled), ("src", Prop::Src), ("alt", Prop::Alt),
            ],
            "textarea" => &[
                ("value", Prop::Value), ("defaultValue", Prop::DefaultValue), ("type", Prop::Type), ("name", Prop::Name),
                ("placeholder", Prop::Placeholder), ("readOnly", Prop::ReadOnly), ("required", Prop::Required),
                ("disabled", Prop::Disabled),
            ],
            "select" => &[
                ("value", Prop::Value), ("selectedIndex", Prop::SelectedIndex), ("options", Prop::Options),
                ("type", Prop::Type), ("multiple", Prop::Multiple), ("name", Prop::Name), ("required", Prop::Required),
                ("disabled", Prop::Disabled),
            ],
            "option" => &[
                ("value", Prop::Value), ("text", Prop::Text), ("selected", Prop::Selected),
                ("defaultSelected", Prop::DefaultSelected), ("disabled", Prop::Disabled),
            ],
            "button" => &[("value", Prop::Value), ("type", Prop::Type), ("name", Prop::Name), ("disabled", Prop::Disabled)],
            "a" => &[("href", Prop::Href), ("type", Prop::Type)],
            "img" => &[("src", Prop::Src), ("alt", Prop::Alt), ("name", Prop::Name)],
            "label" => &[("htmlFor", Prop::HtmlFor)],
            "fieldset" => &[("disabled", Prop::Disabled), ("name", Prop::Name)],
            "iframe" => &[("name", Prop::Name), ("src", Prop::Src)],
            _ => &[],
        };
        allowed.iter().find(|(n, _)| *n == name).map(|(_, p)| *p)
    }

    /// `name in node` for the supported set (see the module comment).
    pub fn has_property(&self, id: NodeId, name: &str) -> bool {
        self.live(id).is_some() && self.prop_of(id, name).is_some()
    }

    /// The property value. Undefined for names outside the set; UnsupportedProperty for object-valued properties.
    pub fn get_property(&mut self, id: NodeId, name: &str) -> Result<HostValue, DomError> {
        self.check(id)?;
        let Some(prop) = self.prop_of(id, name) else { return Ok(HostValue::Undefined) };
        let s = |v: String| HostValue::Str(v);
        Ok(match prop {
            Prop::NodeName => s(self.node_name(id).unwrap_or_default()),
            Prop::NodeType => HostValue::Number(match self.n(id).kind {
                NodeKind::Element => 1.0,
                NodeKind::Text => 3.0,
                NodeKind::Comment => 8.0,
                NodeKind::Document => 9.0,
            }),
            Prop::IsConnected => HostValue::Bool(self.is_connected(id)),
            Prop::TextContent => self.text_content(id).map_or(HostValue::Null, s),
            Prop::NodeValue => self.node_value(id).map_or(HostValue::Null, s),
            Prop::LocalName => s(self.local_name(id).unwrap_or_default().to_string()),
            Prop::TagName => s(self.tag_name(id).unwrap_or_default()),
            Prop::NamespaceUri => s(self.namespace(id).map(|ns| ns.uri().to_string()).unwrap_or_default()),
            Prop::InnerHtml => self.inner_html(id).map_or(HostValue::Undefined, s),
            Prop::OuterHtml => self.outer_html(id).map_or(HostValue::Undefined, s),
            Prop::Id | Prop::ClassName | Prop::Title | Prop::Name | Prop::Placeholder | Prop::Min | Prop::Max
            | Prop::Step | Prop::Href | Prop::Src | Prop::Alt | Prop::HtmlFor => {
                let attr = prop.string_attr().unwrap_or_default();
                s(self.get_attribute(id, attr)?.unwrap_or_default())
            }
            Prop::Hidden | Prop::Disabled | Prop::ReadOnly | Prop::Required | Prop::Multiple | Prop::DefaultChecked
            | Prop::DefaultSelected => {
                let attr = prop.bool_attr().unwrap_or_default();
                HostValue::Bool(self.get_attribute(id, attr)?.is_some())
            }
            Prop::TabIndex => HostValue::Number(f64::from(self.tab_index(id))),
            Prop::Value => HostValue::Str(self.value_of(id)),
            Prop::DefaultValue => HostValue::Str(self.default_value_of(id)),
            Prop::Checked => HostValue::Bool(self.n(id).form.checked),
            Prop::Selected => HostValue::Bool(self.n(id).form.selected),
            Prop::SelectedIndex => HostValue::Number(f64::from(self.select_index(id))),
            Prop::Text => HostValue::Str(self.option_text(id)),
            Prop::Type => HostValue::Str(self.type_of(id)),
            Prop::ParentNode | Prop::FirstChild | Prop::ChildNodes | Prop::Children | Prop::Style | Prop::Options => {
                return Err(DomError::UnsupportedProperty)
            }
        })
    }

    /// The property assignment, with the IDL conversions (DOMString, boolean, long, LegacyNullToEmptyString).
    pub fn set_property(&mut self, id: NodeId, name: &str, value: HostValue, inv: &mut Invoke<'_>) -> Result<(), DomError> {
        self.check(id)?;
        let Some(prop) = self.prop_of(id, name) else { return Err(DomError::UnsupportedProperty) };
        if let Some(attr) = prop.string_attr() {
            return self.set_attribute(id, attr, &value.to_dom_string());
        }
        if let Some(attr) = prop.bool_attr() {
            if value.to_boolean() {
                return self.set_attribute(id, attr, "");
            }
            return self.remove_attribute(id, attr);
        }
        match prop {
            Prop::NodeValue => self.set_node_value(id, &value.to_nullable_string()),
            Prop::TextContent => self.set_text_content(id, &value.to_nullable_string(), inv),
            Prop::Text => self.set_text_content(id, &value.to_dom_string(), inv),
            Prop::TabIndex => self.set_attribute(id, "tabindex", &number_to_string(f64::from(value.to_long()))),
            Prop::Value => self.set_value_of(id, &value),
            Prop::DefaultValue => match self.n(id).kind {
                NodeKind::Element if self.is_html_element_named(id, "textarea") => {
                    self.set_text_content(id, &value.to_dom_string_null_empty(), inv)
                }
                _ => self.set_attribute(id, "value", &value.to_dom_string()),
            },
            Prop::Checked => {
                self.set_checked(id, value.to_boolean());
                Ok(())
            }
            Prop::Selected => {
                self.set_option_selected(id, value.to_boolean());
                Ok(())
            }
            Prop::SelectedIndex => {
                self.set_select_index(id, value.to_long());
                Ok(())
            }
            Prop::Type => {
                if self.is_html_input(id) || self.is_html_element_named(id, "button") || self.is_html_element_named(id, "a") {
                    self.set_attribute(id, "type", &value.to_dom_string())
                } else {
                    Err(DomError::ReadOnlyProperty)
                }
            }
            Prop::Style => self.set_css_text(id, &value.to_dom_string()),
            Prop::InnerHtml | Prop::OuterHtml => Err(DomError::NotSupportedError),
            _ => Err(DomError::ReadOnlyProperty),
        }
    }

    fn value_of(&self, id: NodeId) -> String {
        if self.is_html_input(id) {
            self.input_value(id)
        } else if self.is_html_element_named(id, "textarea") {
            self.textarea_value(id)
        } else if self.is_html_element_named(id, "select") {
            self.select_value(id)
        } else if self.is_html_option(id) {
            self.option_value(id)
        } else {
            self.attr_raw(id, "value").unwrap_or_default()
        }
    }

    fn default_value_of(&self, id: NodeId) -> String {
        if self.is_html_element_named(id, "textarea") {
            self.text_content(id).unwrap_or_default()
        } else {
            self.attr_raw(id, "value").unwrap_or_default()
        }
    }

    fn set_value_of(&mut self, id: NodeId, value: &HostValue) -> Result<(), DomError> {
        if self.is_html_input(id) {
            self.set_input_value(id, &value.to_dom_string_null_empty())
        } else if self.is_html_element_named(id, "textarea") {
            self.set_textarea_value(id, &value.to_dom_string_null_empty());
            Ok(())
        } else if self.is_html_element_named(id, "select") {
            self.set_select_value(id, &value.to_dom_string_null_empty());
            Ok(())
        } else {
            // option and button: the value attribute.
            self.set_attribute(id, "value", &value.to_dom_string_null_empty())
        }
    }

    /// The `type` IDL value: normalized for inputs and buttons, the attribute for links, the fixed name for textarea
    /// and select.
    fn type_of(&self, id: NodeId) -> String {
        if self.is_html_input(id) {
            self.input_type_name(id).unwrap_or("text").to_string()
        } else if self.is_html_element_named(id, "button") {
            self.button_type(id).to_string()
        } else if self.is_html_element_named(id, "textarea") {
            "textarea".to_string()
        } else if self.is_html_element_named(id, "select") {
            if self.attr_raw(id, "multiple").is_some() { "select-multiple" } else { "select-one" }.to_string()
        } else {
            self.attr_raw(id, "type").unwrap_or_default()
        }
    }
}
