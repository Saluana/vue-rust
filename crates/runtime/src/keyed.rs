//! vn-runtime::keyed: the children diff of @vue/runtime-core 3.5.43 (production build): patchUnkeyedChildren,
//! patchKeyedChildren (the five-step algorithm with the longest increasing subsequence of getSequence) and the key map.
//! Keys are compared with SameValueZero, as the JS Map of runtime-core does.

use std::collections::HashMap;

use vn_value::{ObjRef, Res, Sym, Value, JsStr};

use crate::host::{HostNode, HostNs};
use crate::renderer::{array_len, el_of, key_of, vnode_at, Renderer};
use crate::vnode;

/// A key of the Map (SameValueZero): nullish keys are never stored.
#[derive(PartialEq, Eq, Hash)]
enum MapKey {
    Bool(bool),
    Num(u64),
    Str(JsStr),
    Sym(Sym),
    Obj(ObjRef),
}

fn map_key(v: &Value) -> Option<MapKey> {
    Some(match v {
        Value::Bool(b) => MapKey::Bool(*b),
        Value::Number(n) => {
            let bits = if n.is_nan() {
                f64::NAN.to_bits()
            } else if *n == 0.0 {
                0.0f64.to_bits()
            } else {
                n.to_bits()
            };
            MapKey::Num(bits)
        }
        Value::String(s) => MapKey::Str(s.clone()),
        Value::Symbol(s) => MapKey::Sym(*s),
        Value::Object(o) => MapKey::Obj(*o),
        _ => return None,
    })
}

/// `getSequence(arr)`: the indices of one longest increasing subsequence of the non-zero entries of `arr`.
pub(crate) fn get_sequence(arr: &[usize]) -> Vec<usize> {
    let mut p = arr.to_vec();
    let mut result: Vec<usize> = vec![0];
    for i in 0..arr.len() {
        let arr_i = arr[i];
        if arr_i != 0 {
            let j = *result.last().expect("result is never empty");
            if arr[j] < arr_i {
                p[i] = j;
                result.push(i);
                continue;
            }
            let (mut u, mut v) = (0usize, result.len() - 1);
            while u < v {
                let c = (u + v) >> 1;
                if arr[result[c]] < arr_i {
                    u = c + 1;
                } else {
                    v = c;
                }
            }
            if arr_i < arr[result[u]] {
                if u > 0 {
                    p[i] = result[u - 1];
                }
                result[u] = i;
            }
        }
    }
    let mut u = result.len();
    let mut v = result[u - 1];
    while u > 0 {
        u -= 1;
        result[u] = v;
        v = p[v];
    }
    result
}

impl Renderer {
    /// `patchUnkeyedChildren`: the common prefix is patched in place, the rest mounted or unmounted.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn patch_unkeyed_children(
        &self,
        c1: &Value,
        c2: &Value,
        container: HostNode,
        anchor: Option<HostNode>,
        parent: Option<ObjRef>,
        ns: HostNs,
        ssi: &Value,
        optimized: bool,
    ) -> Res<()> {
        let rt = self.rt();
        let old_len = array_len(rt, c1)?;
        let new_len = array_len(rt, c2)?;
        let common = old_len.min(new_len);
        for i in 0..common {
            let next = self.norm_at(c2, i, optimized)?;
            let prev = vnode_at(rt, c1, i)?;
            self.patch(Some(prev), next, container, None, parent, ns, ssi, optimized)?;
        }
        if old_len > new_len {
            self.unmount_children(c1, parent, true, false, common)
        } else {
            self.mount_children(c2, container, anchor, parent, ns, ssi, optimized, common)
        }
    }

    /// `patchKeyedChildren(c1, c2, container, parentAnchor, ...)`.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn patch_keyed_children(
        &self,
        c1: &Value,
        c2: &Value,
        container: HostNode,
        parent_anchor: Option<HostNode>,
        parent: Option<ObjRef>,
        ns: HostNs,
        ssi: &Value,
        optimized: bool,
    ) -> Res<()> {
        let rt = self.rt();
        let l2 = array_len(rt, c2)? as isize;
        let mut i: isize = 0;
        let mut e1: isize = array_len(rt, c1)? as isize - 1;
        let mut e2: isize = l2 - 1;
        // 1. sync from the start
        while i <= e1 && i <= e2 {
            let n1 = vnode_at(rt, c1, i as usize)?;
            let n2 = self.norm_at(c2, i as usize, optimized)?;
            if !vnode::is_same_vnode_type(rt, n1, n2) {
                break;
            }
            self.patch(Some(n1), n2, container, None, parent, ns, ssi, optimized)?;
            i += 1;
        }
        // 2. sync from the end
        while i <= e1 && i <= e2 {
            let n1 = vnode_at(rt, c1, e1 as usize)?;
            let n2 = self.norm_at(c2, e2 as usize, optimized)?;
            if !vnode::is_same_vnode_type(rt, n1, n2) {
                break;
            }
            self.patch(Some(n1), n2, container, None, parent, ns, ssi, optimized)?;
            e1 -= 1;
            e2 -= 1;
        }
        if i > e1 {
            // 3. the rest is new
            if i <= e2 {
                let next_pos = e2 + 1;
                let anchor = if next_pos < l2 { el_of(rt, vnode_at(rt, c2, next_pos as usize)?) } else { parent_anchor };
                while i <= e2 {
                    let n = self.norm_at(c2, i as usize, optimized)?;
                    self.patch(None, n, container, anchor, parent, ns, ssi, optimized)?;
                    i += 1;
                }
            }
        } else if i > e2 {
            // 4. the rest is removed
            while i <= e1 {
                let n = vnode_at(rt, c1, i as usize)?;
                self.unmount(n, parent, true, false)?;
                i += 1;
            }
        } else {
            // 5. unknown sequence: map the new keys, patch or unmount the old ones, move and mount the rest
            let (s1, s2) = (i as usize, i as usize);
            let (e1u, e2u) = (e1 as usize, e2 as usize);
            let mut key_map: HashMap<MapKey, usize> = HashMap::new();
            for ii in s2..=e2u {
                let next = self.norm_at(c2, ii, optimized)?;
                if let Some(k) = map_key(&key_of(rt, next)) {
                    key_map.insert(k, ii);
                }
            }
            let mut patched: usize = 0;
            let to_be_patched = e2u - s2 + 1;
            let mut moved = false;
            let mut max_new_index_so_far: usize = 0;
            let mut new_index_to_old = vec![0usize; to_be_patched];
            for ii in s1..=e1u {
                let prev = vnode_at(rt, c1, ii)?;
                if patched >= to_be_patched {
                    self.unmount(prev, parent, true, false)?;
                    continue;
                }
                let prev_key = key_of(rt, prev);
                let new_index: Option<usize> = if !prev_key.is_nullish() {
                    map_key(&prev_key).and_then(|k| key_map.get(&k).copied())
                } else {
                    let mut found = None;
                    for j in s2..=e2u {
                        let cj = vnode_at(rt, c2, j)?;
                        if new_index_to_old[j - s2] == 0 && vnode::is_same_vnode_type(rt, prev, cj) {
                            found = Some(j);
                            break;
                        }
                    }
                    found
                };
                match new_index {
                    None => self.unmount(prev, parent, true, false)?,
                    Some(ni) => {
                        new_index_to_old[ni - s2] = ii + 1;
                        if ni >= max_new_index_so_far {
                            max_new_index_so_far = ni;
                        } else {
                            moved = true;
                        }
                        let target = vnode_at(rt, c2, ni)?;
                        self.patch(Some(prev), target, container, None, parent, ns, ssi, optimized)?;
                        patched += 1;
                    }
                }
            }
            let increasing = if moved { get_sequence(&new_index_to_old) } else { Vec::new() };
            let mut j: isize = increasing.len() as isize - 1;
            for ii in (0..to_be_patched).rev() {
                let next_index = s2 + ii;
                let next_child = vnode_at(rt, c2, next_index)?;
                let anchor = if (next_index + 1) < l2 as usize {
                    let av = vnode_at(rt, c2, next_index + 1)?;
                    match el_of(rt, av) {
                        Some(e) => Some(e),
                        None => return Err(rt.throw_type_error("unsupported: async component placeholder anchor")),
                    }
                } else {
                    parent_anchor
                };
                if new_index_to_old[ii] == 0 {
                    self.patch(None, next_child, container, anchor, parent, ns, ssi, optimized)?;
                } else if moved {
                    if j < 0 || ii != increasing[j as usize] {
                        self.move_vnode(next_child, container, anchor, 2)?;
                    } else {
                        j -= 1;
                    }
                }
            }
        }
        Ok(())
    }
}
