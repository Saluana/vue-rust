//! Array sort for `Array.prototype.sort` and `toSorted`. For fewer than 64 elements the comparator calls follow V8
//! exactly (checked against Node 26 logs). Array.prototype.sort is a plain binary insertion sort below 8 elements and a
//! run detection followed by binary insertion from 8 on; toSorted detects the initial run for every size. Larger arrays
//! (64 and more) use a stable merge sort, which gives the same result for a consistent comparator but may call the
//! comparator in another order (ABI.md section 16).

use crate::value::{Res, Value};

/// Sorts `a` with `cmp`, where a negative result puts the first argument first. `to_sorted` selects toSorted's
/// small-array path. An error from `cmp` propagates; the caller then leaves the array unchanged because it writes back
/// only after a successful sort.
pub(crate) fn sort_values(
    a: &mut [Value],
    cmp: &mut dyn FnMut(&Value, &Value) -> Res<f64>,
    to_sorted: bool,
) -> Res<()> {
    let n = a.len();
    if n < 2 {
        return Ok(());
    }
    if n < 64 {
        if n < 8 && !to_sorted {
            return insert_from(a, 1, cmp);
        }
        let run = count_and_make_run(a, cmp)?;
        if run < n {
            insert_from(a, run, cmp)?;
        }
        Ok(())
    } else {
        let mut tmp: Vec<Value> = Vec::with_capacity(n);
        merge_sort(a, &mut tmp, cmp)
    }
}

/// Binary insertion of `a[start..]` into the sorted prefix `a[..start]`: the pivot is compared as cmp(pivot, a[mid]),
/// and a result below zero moves the search left, so equal elements keep their order.
fn insert_from(a: &mut [Value], mut start: usize, cmp: &mut dyn FnMut(&Value, &Value) -> Res<f64>) -> Res<()> {
    let n = a.len();
    while start < n {
        let pivot = a[start].clone();
        let (mut left, mut right) = (0usize, start);
        while left < right {
            let mid = left + ((right - left) >> 1);
            if cmp(&pivot, &a[mid])? < 0.0 {
                right = mid;
            } else {
                left = mid + 1;
            }
        }
        a[left..=start].rotate_right(1);
        start += 1;
    }
    Ok(())
}

/// The length of the run at the start of `a` (n >= 2). A strictly descending run is reversed in place. Each element is
/// compared with its predecessor, as V8's CountAndMakeRun does.
fn count_and_make_run(a: &mut [Value], cmp: &mut dyn FnMut(&Value, &Value) -> Res<f64>) -> Res<usize> {
    let n = a.len();
    let mut run = 2;
    let descending = cmp(&a[1], &a[0])? < 0.0;
    let mut idx = 2;
    while idx < n {
        let order = cmp(&a[idx], &a[idx - 1])?;
        if descending {
            if order >= 0.0 {
                break;
            }
        } else if order < 0.0 {
            break;
        }
        run += 1;
        idx += 1;
    }
    if descending {
        a[..run].reverse();
    }
    Ok(run)
}

/// Stable top-down merge sort for 64 or more elements; leaves of up to 16 elements use binary insertion.
fn merge_sort(a: &mut [Value], tmp: &mut Vec<Value>, cmp: &mut dyn FnMut(&Value, &Value) -> Res<f64>) -> Res<()> {
    let n = a.len();
    if n <= 16 {
        return insert_from(a, 1, cmp);
    }
    let mid = n / 2;
    merge_sort(&mut a[..mid], tmp, cmp)?;
    merge_sort(&mut a[mid..], tmp, cmp)?;
    if cmp(&a[mid], &a[mid - 1])? >= 0.0 {
        return Ok(());
    }
    tmp.clear();
    let (mut i, mut j) = (0usize, mid);
    while i < mid && j < n {
        if cmp(&a[j], &a[i])? < 0.0 {
            tmp.push(a[j].clone());
            j += 1;
        } else {
            tmp.push(a[i].clone());
            i += 1;
        }
    }
    tmp.extend_from_slice(&a[i..mid]);
    tmp.extend_from_slice(&a[j..n]);
    a.clone_from_slice(tmp);
    Ok(())
}
