//! vn-runtime::ssr: `renderToString` of @vue/server-renderer 3.5.43 (its vnode path: renderVNode, renderElementVNode,
//! renderComponentVNode, renderComponentSubTree, ssrRenderAttrs, applySSRDirectives) for the probe subset. The output is a
//! string built from the same vnodes and the same component setup and render code as the client renderer; nothing is
//! mounted, and no DOM node is created. Escaping follows @vue/shared (`escapeHtml`, `escapeHtmlComment`), which is not
//! the DOM serializer's escaping: an apostrophe becomes `&#39;`.
//! Fragments are wrapped in `<!--[-->` and `<!--]-->`; an empty comment vnode renders `<!---->`.
//! Not in the subset (TypeError): SVG and MathML, scope ids, Teleport, Suspense, Transition, KeepAlive, async components,
//! functional components, compiled `ssrRender` functions, and `getSSRProps` of directives other than v-model text.

use vn_value::{ObjRef, Res, Rt, Value};

use crate::dom_props::is_on;
use crate::normalize::{array_get, for_in_keys, length_of};
use crate::renderer::{array_len, children_of, dirs_of, is_type, key_string, props_of, scope_id_of, shape_of, ty_of, Renderer};
use crate::shapes::ShapeFlags;

/// `escapeHtml` of @vue/shared: `"`, `&`, `'`, `<` and `>` become character references.
pub fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("&quot;"),
            '&' => out.push_str("&amp;"),
            '\'' => out.push_str("&#39;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            c => out.push(c),
        }
    }
    out
}

/// The length of the run of `>` and `->` at the start of `s`: the `^(?:-?>)+` alternative of `commentStripRE`.
fn leading_gts(s: &str) -> usize {
    let mut j = 0;
    loop {
        if s[j..].starts_with("->") {
            j += 2;
        } else if s[j..].starts_with('>') {
            j += 1;
        } else {
            return j;
        }
    }
}

/// One pass of `commentStripRE = /^(?:-?>)+|<!--|-->|--!>|<!-$/g` over `src`, scanned left to right. The first alternative
/// only matches at the start of the string; the last one only at its end.
fn strip_comment_delimiters(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    while i < src.len() {
        let rest = &src[i..];
        if i == 0 {
            let lead = leading_gts(rest);
            if lead > 0 {
                i += lead;
                continue;
            }
        }
        let delimiter = if rest.starts_with("<!--") {
            4
        } else if rest.starts_with("-->") {
            3
        } else if rest.starts_with("--!>") {
            4
        } else if rest == "<!-" {
            3
        } else {
            0
        };
        if delimiter > 0 {
            i += delimiter;
            continue;
        }
        let Some(c) = rest.chars().next() else { break };
        out.push(c);
        i += c.len_utf8();
    }
    out
}

/// `escapeHtmlComment` of @vue/shared: strips the comment delimiters until a pass changes nothing.
pub fn escape_html_comment(src: &str) -> String {
    let mut cur = src.to_string();
    loop {
        let next = strip_comment_delimiters(&cur);
        if next == cur {
            return cur;
        }
        cur = next;
    }
}

/// `hyphenate` of @vue/shared: an ASCII capital after a word character gets a hyphen, then the whole text is lowercased.
fn hyphenate(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len() + 4);
    for (i, c) in chars.iter().enumerate() {
        if c.is_ascii_uppercase() && i > 0 && (chars[i - 1].is_ascii_alphanumeric() || chars[i - 1] == '_') {
            out.push('-');
        }
        out.extend(c.to_lowercase());
    }
    out
}

/// `isBooleanAttr` of @vue/shared.
fn is_boolean_attr(name: &str) -> bool {
    matches!(
        name,
        "itemscope" | "allowfullscreen" | "formnovalidate" | "ismap" | "nomodule" | "novalidate" | "readonly" | "async"
            | "autofocus" | "autoplay" | "controls" | "default" | "defer" | "disabled" | "inert" | "loop" | "open"
            | "required" | "reversed" | "scoped" | "seamless" | "checked" | "muted" | "multiple" | "selected"
    )
}

/// `isVoidTag` of @vue/shared.
fn is_void_tag(tag: &str) -> bool {
    matches!(
        tag,
        "area" | "base" | "br" | "col" | "embed" | "hr" | "img" | "input" | "link" | "meta" | "param" | "source" | "track" | "wbr"
    )
}

/// `isSSRSafeAttrName`: no `>`, `/`, `=`, quotes or ASCII whitespace.
fn is_ssr_safe_attr_name(name: &str) -> bool {
    !name.chars().any(|c| matches!(c, '>' | '/' | '=' | '"' | '\'' | '\t' | '\n' | '\u{c}' | '\r' | ' '))
}

/// `propsToAttrMap` of @vue/shared.
fn props_to_attr(key: &str) -> Option<&'static str> {
    match key {
        "acceptCharset" => Some("accept-charset"),
        "className" => Some("class"),
        "htmlFor" => Some("for"),
        "httpEquiv" => Some("http-equiv"),
        _ => None,
    }
}

impl Renderer {
    /// `renderToString` of one vnode (renderVNode): appends its HTML to `out`. `parent` is the component whose subtree
    /// holds the vnode (the `parentComponent` of server-renderer).
    pub(crate) fn ssr_vnode(&self, out: &mut String, vnode: ObjRef, parent: Option<ObjRef>) -> Res<()> {
        let rt = self.rt();
        let ty = ty_of(rt, vnode);
        let shape = shape_of(rt, vnode);
        if is_type(rt, &ty, "v-txt") {
            let text = self.to_s(&children_of(rt, vnode))?;
            out.push_str(&escape_html(&text));
        } else if is_type(rt, &ty, "v-cmt") {
            let c = children_of(rt, vnode);
            if c.to_boolean() {
                let text = self.to_s(&c)?;
                out.push_str("<!--");
                out.push_str(&escape_html_comment(&text));
                out.push_str("-->");
            } else {
                out.push_str("<!---->");
            }
        } else if is_type(rt, &ty, "v-stc") {
            return Err(rt.throw_type_error("unsupported: Static (hoisted) vnode in SSR"));
        } else if is_type(rt, &ty, "v-fgt") {
            if scope_id_of(rt, vnode).to_boolean() {
                return Err(rt.throw_type_error("unsupported: scope ids in SSR"));
            }
            out.push_str("<!--[-->");
            self.ssr_children(out, &children_of(rt, vnode), parent)?;
            out.push_str("<!--]-->");
        } else if shape & ShapeFlags::ELEMENT != 0 {
            self.ssr_element(out, vnode, parent)?;
        } else if shape & ShapeFlags::COMPONENT != 0 {
            self.ssr_component(out, vnode, parent)?;
        } else if shape & ShapeFlags::TELEPORT != 0 {
            return Err(rt.throw_type_error("unsupported: Teleport in SSR"));
        } else if shape & ShapeFlags::SUSPENSE != 0 {
            return Err(rt.throw_type_error("unsupported: Suspense in SSR"));
        } else {
            return Err(rt.throw_type_error("[@vue/server-renderer] Invalid VNode type"));
        }
        Ok(())
    }

    /// `renderVNodeChildren`: each child normalized (strings become text vnodes, null becomes a comment vnode).
    fn ssr_children(&self, out: &mut String, children: &Value, parent: Option<ObjRef>) -> Res<()> {
        let rt = self.rt();
        for i in 0..array_len(rt, children)? {
            let raw = array_get(rt, children, i)?;
            let child = self.norm_value(&raw, false)?;
            self.ssr_vnode(out, child, parent)?;
        }
        Ok(())
    }

    /// `renderElementVNode`: the open tag with its attributes, the children (or innerHTML/textContent) and the end tag.
    fn ssr_element(&self, out: &mut String, vnode: ObjRef, parent: Option<ObjRef>) -> Res<()> {
        let rt = self.rt();
        let tag = self.to_s(&ty_of(rt, vnode))?;
        if tag == "svg" || tag == "math" {
            return Err(rt.throw_type_error("unsupported: SVG and MathML in SSR"));
        }
        let props = self.ssr_props(vnode)?;
        out.push('<');
        out.push_str(&tag);
        if props.to_boolean() {
            out.push_str(&self.ssr_attrs(&props, &tag)?);
        }
        out.push('>');
        if is_void_tag(&tag) {
            return Ok(());
        }
        let mut has_override = false;
        if props.to_boolean() {
            let inner = rt.get_str(&props, "innerHTML")?;
            let text = if inner.to_boolean() { None } else { Some(rt.get_str(&props, "textContent")?) };
            if inner.to_boolean() {
                has_override = true;
                out.push_str(&self.to_s(&inner)?);
            } else if let Some(t) = text.filter(|t| t.to_boolean()) {
                has_override = true;
                out.push_str(&escape_html(&self.to_s(&t)?));
            } else if tag == "textarea" {
                let v = rt.get_str(&props, "value")?;
                if v.to_boolean() {
                    has_override = true;
                    out.push_str(&escape_html(&self.to_s(&v)?));
                }
            }
        }
        if !has_override {
            let shape = shape_of(rt, vnode);
            let children = children_of(rt, vnode);
            if shape & ShapeFlags::TEXT_CHILDREN != 0 {
                out.push_str(&escape_html(&self.to_s(&children)?));
            } else if shape & ShapeFlags::ARRAY_CHILDREN != 0 {
                self.ssr_children(out, &children, parent)?;
            }
        }
        out.push_str("</");
        out.push_str(&tag);
        out.push('>');
        Ok(())
    }

    /// `applySSRDirectives`: the element's props merged with what each directive's `getSSRProps` returns (v-model text
    /// contributes `value`). Only element vnodes are merged; the vnode's own props are left as they are.
    fn ssr_props(&self, vnode: ObjRef) -> Res<Value> {
        let rt = self.rt();
        let props = props_of(rt, vnode);
        let dirs = dirs_of(rt, vnode);
        if !dirs.to_boolean() {
            return Ok(props);
        }
        let mut args = vec![if props.to_boolean() { props } else { Value::Object(rt.new_object()) }];
        for i in 0..array_len(rt, &dirs)? {
            let binding = array_get(rt, &dirs, i)?;
            let dir = rt.get_str(&binding, "dir")?;
            let get = rt.get_str(&dir, "getSSRProps")?;
            if rt.is_callable(&get) {
                let r = rt.call(&get, &Value::Undefined, &[binding.clone(), Value::Object(vnode)])?;
                if r.to_boolean() {
                    args.push(r);
                }
            }
        }
        Ok(Value::Object(crate::vnode::merge_props(rt, &args)?))
    }

    /// `ssrRenderAttrs(props, tag)`.
    fn ssr_attrs(&self, props: &Value, tag: &str) -> Res<String> {
        let rt = self.rt();
        let mut ret = String::new();
        for key in for_in_keys(rt, props)? {
            let mut k = key_string(&key);
            if matches!(k.as_str(), "key" | "ref" | "innerHTML" | "textContent" | "ref_key" | "ref_for")
                || is_on(&k)
                || (tag == "textarea" && k == "value")
                || k.starts_with('.')
            {
                continue;
            }
            let value = rt.get_v(props, &key)?;
            if let Some(rest) = k.strip_prefix('^') {
                k = rest.to_string();
            }
            if k == "class" {
                let c = self.normalize_class(&value)?;
                ret.push_str(&format!(" class=\"{}\"", escape_html(&c)));
            } else if k == "style" {
                let s = self.render_style(&value)?;
                ret.push_str(&format!(" style=\"{}\"", escape_html(&s)));
            } else if k == "className" {
                if !value.is_nullish() {
                    let c = self.to_s(&value)?;
                    ret.push_str(&format!(" class=\"{}\"", escape_html(&c)));
                }
            } else {
                ret.push_str(&self.ssr_dynamic_attr(&k, &value, tag)?);
            }
        }
        Ok(ret)
    }

    /// `ssrRenderDynamicAttr(key, value, tag)`.
    fn ssr_dynamic_attr(&self, key: &str, value: &Value, tag: &str) -> Res<String> {
        let renderable = match value {
            Value::Bool(_) | Value::Number(_) | Value::String(_) => true,
            _ => false,
        };
        if !renderable {
            return Ok(String::new());
        }
        let attr_key = if tag.contains('-') {
            key.to_string()
        } else {
            props_to_attr(key).map(str::to_string).unwrap_or_else(|| key.to_ascii_lowercase())
        };
        let hidden_flag = attr_key == "hidden" && matches!(value, Value::Bool(_) | Value::Number(_));
        if is_boolean_attr(&attr_key) || hidden_flag {
            let include = value.to_boolean() || matches!(value, Value::String(s) if s.is_empty());
            return Ok(if include { format!(" {attr_key}") } else { String::new() });
        }
        if is_ssr_safe_attr_name(&attr_key) {
            let text = self.to_s(value)?;
            let empty_string = matches!(value, Value::String(s) if s.is_empty());
            return Ok(if empty_string { format!(" {attr_key}") } else { format!(" {attr_key}=\"{}\"", escape_html(&text)) });
        }
        // Vue logs an error and skips the attribute.
        Ok(String::new())
    }

    /// `normalizeClass` of @vue/shared: strings as they are, arrays and objects flattened to their truthy names.
    fn normalize_class(&self, value: &Value) -> Res<String> {
        let rt = self.rt();
        let mut res = String::new();
        match value {
            Value::String(s) => res = s.to_rust_string_lossy(),
            Value::Object(_) if rt.is_array(value) => {
                for i in 0..length_of(rt, value)? as usize {
                    let item = array_get(rt, value, i)?;
                    let n = self.normalize_class(&item)?;
                    if !n.is_empty() {
                        res.push_str(&n);
                        res.push(' ');
                    }
                }
            }
            Value::Object(_) => {
                for key in for_in_keys(rt, value)? {
                    if rt.get_v(value, &key)?.to_boolean() {
                        res.push_str(&key_string(&key));
                        res.push(' ');
                    }
                }
            }
            _ => {}
        }
        Ok(res.trim().to_string())
    }

    /// `ssrRenderStyle`: a string as it is; an object as `stringifyStyle` writes it (string and number values only).
    fn render_style(&self, value: &Value) -> Res<String> {
        let rt = self.rt();
        if !value.to_boolean() {
            return Ok(String::new());
        }
        if let Value::String(s) = value {
            return Ok(s.to_rust_string_lossy());
        }
        if rt.is_array(value) {
            return Err(rt.throw_type_error("unsupported: style arrays in SSR"));
        }
        let mut out = String::new();
        for key in for_in_keys(rt, value)? {
            let name = key_string(&key);
            if name.starts_with(":--") {
                return Err(rt.throw_type_error("unsupported: CSS variables in style objects in SSR"));
            }
            let v = rt.get_v(value, &key)?;
            if matches!(v, Value::String(_) | Value::Number(_)) {
                let normalized = if name.starts_with("--") { name.clone() } else { hyphenate(&name) };
                out.push_str(&normalized);
                out.push(':');
                out.push_str(&self.to_s(&v)?);
                out.push(';');
            }
        }
        Ok(out)
    }

    /// `ssrRenderComponent` / `renderComponentVNode` + `renderComponentSubTree` for a stateful component.
    fn ssr_component(&self, out: &mut String, vnode: ObjRef, parent: Option<ObjRef>) -> Res<()> {
        let rt = self.rt();
        let shape = shape_of(rt, vnode);
        if shape & ShapeFlags::FUNCTIONAL_COMPONENT != 0 {
            return Err(rt.throw_type_error("unsupported: functional components in SSR"));
        }
        self.reject_builtin(vnode, "SSR")?;
        let inst = self.create_instance_for(vnode, parent, false)?;
        let ty = crate::instance::instance_of(rt, inst).ty.borrow().clone();
        if rt.get_str(&ty, "ssrRender")?.to_boolean() || rt.get_str(&ty, "__ssrInlineRender")?.to_boolean() {
            return Err(rt.throw_type_error("unsupported: compiled ssrRender functions in SSR"));
        }
        let sub = self.render_component_root(inst)?;
        *crate::instance::instance_of(rt, inst).sub_tree.borrow_mut() = Value::Object(sub);
        self.ssr_vnode(out, sub, Some(inst))
    }
}

/// `renderToString(vnode)` of @vue/server-renderer for the realm: the HTML of a root vnode, e.g. `createVNode(Component)`.
/// The realm's renderer must be installed (`renderer::install`); no DOM node is created by this call.
pub fn render_to_string(rt: &Rt, root: ObjRef) -> Res<String> {
    let r = crate::renderer::renderer(rt)?;
    let mut out = String::new();
    r.ssr_vnode(&mut out, root, None)?;
    Ok(out)
}

/// `renderToString(app)` of @vue/server-renderer for an app made by `createApp` or `createSSRApp`: the app's root
/// component is rendered as a vnode with the app's context (`createVNode(app._component, app._props)`), so `provide`
/// and the app-level registries reach the subtree as they do on the client.
pub fn render_app_to_string(rt: &Rt, app: &Value) -> Res<String> {
    let root = rt.get_str(app, "_component")?;
    let props = rt.get_str(app, "_props")?;
    let context = rt.get_str(app, "_context")?;
    let vnode = rt.call(&crate::renderer::builtin_fn(rt, "vn-runtime.createVNode"), &Value::Undefined, &[root, props])?;
    let Value::Object(vnode) = vnode else {
        return Err(rt.throw_type_error("createVNode returned no vnode (framework bug)"));
    };
    crate::vnode::fields(rt, vnode, |f| f.app_context = context);
    render_to_string(rt, vnode)
}
