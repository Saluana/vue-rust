//! The registered `EventSink` and the dispatch entry of `vn_event`. No cell is borrowed while the sink runs, so the sink may
//! call back into the host (for example remove the node that is being inserted) without a borrow conflict.

use std::cell::RefCell;
use std::rc::Rc;

use vn_runtime::host::{EventSink, EventInfo, EventKind, HostEvent, ListenerId, ListenerOutcome};

use crate::abi::LOG_WARN;
use crate::ffi;

thread_local! {
    static CONTEXT: RefCell<EventInfo> = const { RefCell::new(EventInfo { serial: 0, kind: EventKind::Event }) };
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

/// Context supplied by the generated application's optional event entry before vn_event.
pub fn set_event_context(serial: u32, kind: u32) {
    let kind = match kind { 1 => EventKind::Mouse, 2 => EventKind::Keyboard, 3 => EventKind::Input, _ => EventKind::Event };
    CONTEXT.with(|slot| *slot.borrow_mut() = EventInfo { serial: u64::from(serial), kind });
}

pub(crate) fn event_info() -> EventInfo { CONTEXT.with(|slot| *slot.borrow()) }

/// Ends the browser dispatch batch before the app's GC safepoint.
pub fn finish_dispatch() {
    let sink = SINK.with(|slot| slot.borrow().clone());
    if let Some(sink) = sink { sink.finish_dispatch(); }
}
