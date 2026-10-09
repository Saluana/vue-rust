//! Event listeners and dispatch (WP-U0). The dispatch follows the DOM event path (capture from the root down, the
//! target, then bubbling when the event bubbles). Listeners run through a callback, `Invoke`, which receives the
//! document so a listener can change the tree. Activation behaviour: a checkbox or radio click is applied before the
//! listeners run and reverted when the event is cancelled; a click on a label forwards to its control. A click that
//! changed checkedness on a connected, non-canceled checkbox or radio then fires input and change (both bubble), the
//! order Chrome shows. Nothing else synthesises input or change; a select or option is not activated.

use crate::error::DomError;
use crate::node::NodeId;
use crate::Document;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ListenerId(pub u32);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ListenerOptions {
    pub capture: bool,
    pub once: bool,
    pub passive: bool,
}

/// The interface an event is created with. Only Mouse click has activation behaviour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventKind {
    Event,
    Mouse,
    Keyboard,
    Input,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EventInit {
    pub ty: String,
    pub kind: EventKind,
    pub bubbles: bool,
    pub cancelable: bool,
    pub composed: bool,
    /// Supplied by the caller (vn-dom has no clock).
    pub time_stamp: f64,
    pub key: String,
    pub code: String,
    pub ctrl_key: bool,
    pub shift_key: bool,
    pub alt_key: bool,
    pub meta_key: bool,
    pub button: i32,
    pub client_x: i32,
    pub client_y: i32,
    pub detail: i32,
    pub input_data: Option<String>,
    pub is_composing: bool,
}

impl EventInit {
    /// A non-bubbling, non-cancelable event with empty payload.
    pub fn new(ty: &str, kind: EventKind) -> EventInit {
        EventInit {
            ty: ty.to_string(),
            kind,
            bubbles: false,
            cancelable: false,
            composed: false,
            time_stamp: 0.0,
            key: String::new(),
            code: String::new(),
            ctrl_key: false,
            shift_key: false,
            alt_key: false,
            meta_key: false,
            button: 0,
            client_x: 0,
            client_y: 0,
            detail: 0,
            input_data: None,
            is_composing: false,
        }
    }
}

/// The state a listener sees and the only place its flags are changed.
#[derive(Debug)]
pub struct EventState {
    pub init: EventInit,
    pub target: NodeId,
    pub current_target: Option<NodeId>,
    /// 0 none, 1 capturing, 2 at target, 3 bubbling (the DOM eventPhase values).
    pub phase: u8,
    stop: bool,
    stop_now: bool,
    canceled: bool,
    in_passive: bool,
}

impl EventState {
    fn new(init: EventInit, target: NodeId) -> EventState {
        EventState { init, target, current_target: None, phase: 0, stop: false, stop_now: false, canceled: false, in_passive: false }
    }

    pub fn stop_propagation(&mut self) {
        self.stop = true;
    }

    pub fn stop_immediate_propagation(&mut self) {
        self.stop = true;
        self.stop_now = true;
    }

    /// Cancels the event only if it is cancelable and the listener is not passive.
    pub fn prevent_default(&mut self) {
        if self.init.cancelable && !self.in_passive {
            self.canceled = true;
        }
    }

    pub fn default_prevented(&self) -> bool {
        self.canceled
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DispatchResult {
    pub default_prevented: bool,
}

/// The listener callback. Returns false when the listener threw (dispatch goes on either way).
pub type Invoke<'a> = dyn FnMut(&mut Document, ListenerId, &mut EventState) -> bool + 'a;

#[derive(Clone, Debug)]
pub(crate) struct Listener {
    pub uid: u64,
    pub ty: String,
    pub id: ListenerId,
    pub capture: bool,
    pub once: bool,
    pub passive: bool,
}

/// What a legacy-pre-activation step changed, so a cancelled click can undo it.
enum Pre {
    None,
    Checkbox { old: bool },
    Radio { old: bool, prev: Option<NodeId> },
}

impl Document {
    /// Adding the same (type, listener, capture) twice is ignored, including its options.
    pub fn add_event_listener(&mut self, node: NodeId, ty: &str, id: ListenerId, opts: ListenerOptions) -> Result<(), DomError> {
        self.check(node)?;
        let dup = self.n(node).listeners.iter().any(|l| l.ty == ty && l.id == id && l.capture == opts.capture);
        if dup {
            return Ok(());
        }
        let uid = self.next_uid;
        self.next_uid += 1;
        let listener = Listener { uid, ty: ty.to_string(), id, capture: opts.capture, once: opts.once, passive: opts.passive };
        self.nm(node).listeners.push(listener);
        Ok(())
    }

    pub fn remove_event_listener(&mut self, node: NodeId, ty: &str, id: ListenerId, capture: bool) -> Result<(), DomError> {
        self.check(node)?;
        self.nm(node).listeners.retain(|l| !(l.ty == ty && l.id == id && l.capture == capture));
        Ok(())
    }

    /// The DOM dispatch algorithm. Errors: StaleNode for a released target.
    pub fn dispatch_event(&mut self, target: NodeId, init: EventInit, inv: &mut Invoke<'_>) -> Result<DispatchResult, DomError> {
        self.check(target)?;
        let path = self.event_path(target);
        let mut ev = EventState::new(init, target);
        let activation = if ev.init.kind == EventKind::Mouse && ev.init.ty == "click" {
            self.activation_target(&path, ev.init.bubbles)
        } else {
            None
        };
        let pre = match activation {
            Some(a) => self.legacy_pre_activation(a),
            None => Pre::None,
        };
        // input and change follow only a click that changed checkedness (an already-checked radio changes nothing).
        let changed = match pre {
            Pre::None => false,
            Pre::Checkbox { .. } => true,
            Pre::Radio { old, .. } => !old,
        };
        for (i, &node) in path.iter().enumerate().rev() {
            if ev.stop {
                break;
            }
            ev.phase = if i == 0 { 2 } else { 1 };
            ev.current_target = Some(node);
            self.invoke_on(node, &mut ev, true, inv);
        }
        for (i, &node) in path.iter().enumerate() {
            if ev.stop {
                break;
            }
            ev.phase = if i == 0 {
                2
            } else if ev.init.bubbles {
                3
            } else {
                continue;
            };
            ev.current_target = Some(node);
            self.invoke_on(node, &mut ev, false, inv);
        }
        ev.phase = 0;
        ev.current_target = None;
        if let Some(a) = activation.filter(|&a| self.live(a).is_some()) {
            if ev.canceled {
                self.legacy_canceled_activation(a, pre);
            } else {
                // A label forwards its click to the control (which then fires its own input and change).
                self.activation_behaviour(a, ev.target, inv)?;
                // After the click has finished dispatching: input, then change, and only if the input is connected.
                if changed && self.is_connected(a) {
                    self.fire_input_and_change(a, inv);
                }
            }
        }
        Ok(DispatchResult { default_prevented: ev.canceled })
    }

    /// click(): does nothing on a disabled form control; otherwise a bubbling, cancelable MouseEvent.
    pub fn click(&mut self, node: NodeId, inv: &mut Invoke<'_>) -> Result<(), DomError> {
        self.check(node)?;
        if self.is_disabled_control(node) {
            return Ok(());
        }
        let mut init = EventInit::new("click", EventKind::Mouse);
        init.bubbles = true;
        init.cancelable = true;
        init.composed = true;
        self.dispatch_event(node, init, inv)?;
        Ok(())
    }

    /// The input event (bubbles, composed) and then the change event (bubbles) of a checkbox or radio.
    fn fire_input_and_change(&mut self, input: NodeId, inv: &mut Invoke<'_>) {
        let mut input_init = EventInit::new("input", EventKind::Input);
        input_init.bubbles = true;
        input_init.composed = true;
        let _ = self.dispatch_event(input, input_init, inv);
        let mut change_init = EventInit::new("change", EventKind::Event);
        change_init.bubbles = true;
        let _ = self.dispatch_event(input, change_init, inv);
    }

    /// Dispatches a plain event from inside vn-dom (focus and blur events).
    pub(crate) fn fire(&mut self, target: NodeId, ty: &str, bubbles: bool, inv: &mut Invoke<'_>) {
        let mut init = EventInit::new(ty, EventKind::Event);
        init.bubbles = bubbles;
        let _ = self.dispatch_event(target, init, inv);
    }

    /// The target followed by its ancestors, the order of the path used by dispatch.
    fn event_path(&self, target: NodeId) -> Vec<NodeId> {
        let mut path = vec![target];
        let mut cur = self.n(target).parent;
        while let Some(p) = cur {
            path.push(p);
            cur = self.n(p).parent;
        }
        path
    }

    /// Runs the listeners of `node` for one pass. The list is snapshotted when the node is reached: listeners added to
    /// this node during the dispatch do not run here, and a listener removed before its turn is skipped.
    fn invoke_on(&mut self, node: NodeId, ev: &mut EventState, capturing: bool, inv: &mut Invoke<'_>) {
        // The node may have been released by an earlier listener on the path: then it has no listeners left to run.
        let Some(live) = self.live(node) else { return };
        let snapshot: Vec<u64> = live
            .listeners
            .iter()
            .filter(|l| l.ty == ev.init.ty && l.capture == capturing)
            .map(|l| l.uid)
            .collect();
        for uid in snapshot {
            if ev.stop_now {
                return;
            }
            // A listener may have released this node through the callback: then there is nothing left to run.
            let Some(l) = self.live(node).and_then(|n| n.listeners.iter().find(|l| l.uid == uid)).cloned() else { continue };
            if l.once {
                self.nm(node).listeners.retain(|x| x.uid != uid);
            }
            ev.in_passive = l.passive;
            let _threw = !inv(self, l.id, ev);
            ev.in_passive = false;
        }
    }

    /// The activation target: the target if it has activation behaviour, else (for bubbling events) the nearest ancestor.
    fn activation_target(&self, path: &[NodeId], bubbles: bool) -> Option<NodeId> {
        for (i, &n) in path.iter().enumerate() {
            if i > 0 && !bubbles {
                break;
            }
            if self.has_activation_behaviour(n) {
                return Some(n);
            }
        }
        None
    }

    fn legacy_pre_activation(&mut self, a: NodeId) -> Pre {
        if self.is_html_input(a) {
            match self.input_type_name(a) {
                Some("checkbox") => {
                    let old = self.n(a).form.checked;
                    self.set_checked(a, !old);
                    return Pre::Checkbox { old };
                }
                Some("radio") => {
                    let old = self.n(a).form.checked;
                    let prev = self.checked_radio_in_group(a);
                    self.set_checked(a, true);
                    return Pre::Radio { old, prev };
                }
                _ => {}
            }
        }
        Pre::None
    }

    fn legacy_canceled_activation(&mut self, a: NodeId, pre: Pre) {
        match pre {
            Pre::None => {}
            Pre::Checkbox { old } => self.nm(a).form.checked = old,
            Pre::Radio { old, prev } => {
                self.nm(a).form.checked = old;
                if let Some(p) = prev {
                    self.nm(p).form.checked = true;
                }
            }
        }
    }

    fn activation_behaviour(&mut self, a: NodeId, target: NodeId, inv: &mut Invoke<'_>) -> Result<(), DomError> {
        if self.is_html_label(a) {
            if let Some(control) = self.label_control(a) {
                if !self.contains(control, target) {
                    self.click(control, inv)?;
                }
            }
        }
        Ok(())
    }

    pub(crate) fn has_activation_behaviour(&self, n: NodeId) -> bool {
        self.is_html_label(n) || (self.is_html_input(n) && matches!(self.input_type_name(n), Some("checkbox") | Some("radio")))
    }
}
