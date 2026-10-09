//! The registered `EventSink` and the dispatch entry of `vn_event`. No cell is borrowed while the sink runs, so the sink may
//! call back into the host (for example remove the node that is being inserted) without a borrow conflict.

use std::cell::RefCell;
use std::rc::Rc;

use vn_runtime::host::{EventSink, HostEvent, ListenerId, ListenerOutcome};

use crate::abi::LOG_WARN;
use crate::ffi;

thread_local! {
    static SINK: RefCell<Option<Rc<dyn EventSink>>> = const { RefCell::new(None) };
}

pub(crate) fn install(sink: Rc<dyn EventSink>) {
    SINK.with(|slot| *slot.borrow_mut() = Some(sink));
}

pub(crate) fn dispatch(listener: ListenerId, event: &HostEvent) -> ListenerOutcome {
    let sink = SINK.with(|slot| slot.borrow().clone());
    match sink {
        Some(sink) => sink.dispatch(listener, event),
        None => {
            ffi::log(LOG_WARN, "event delivered before an event sink was installed");
            ListenerOutcome::default()
        }
    }
}
