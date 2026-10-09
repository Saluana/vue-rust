// Host-only app: mounts one paragraph into the container through the browser host.
mod vn_config;

use vn_renderer_dom::WebHost;
use vn_runtime::host::{Host, HostNs};

#[unsafe(no_mangle)]
pub extern "C" fn vn_start() {
    let mut host = WebHost::new();
    let container = host
        .query_selector(vn_config::CONTAINER)
        .expect("query the container")
        .expect("the container exists");
    let p = host.create_element(HostNs::Html, "p", None).expect("create p");
    host.set_attribute(p, "id", "host-only").expect("set id");
    host.set_element_text(p, "Hello from the browser host").expect("set text");
    host.insert(p, container, None).expect("insert p");
}
