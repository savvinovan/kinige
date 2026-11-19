//! The convergence of details edits from two devices on a row that has none of the groups, in every merge order, with clears.

use super::*;

#[derive(Clone, Copy, Debug)]
enum Op {
    Name(&'static str),
    Describe(&'static str),
    Hide,
    Show,
    ClearName,
    ClearDescription,
}

const OPS: [Op; 7] = [
    Op::Name("a.jpg"),
    Op::Name("b.jpg"),
    Op::Describe("x"),
    Op::Hide,
    Op::Show,
    Op::ClearName,
    Op::ClearDescription,
];

fn field(op: Op) -> (&'static str, serde_json::Value) {
    match op {
        Op::Name(name) => ("name", name_group(Some(name))),
        Op::Describe(text) => ("description", description_group(Some(text))),
        Op::Hide => ("privacy", privacy_group(true)),
        Op::Show => ("privacy", privacy_group(false)),
        Op::ClearName => ("name", name_group(None)),
        Op::ClearDescription => ("description", description_group(None)),
    }
}

/// Whether the edit changes what the row reads as: the store writes nothing for a field that already has the value, which is what keeps a
/// default-valued group (`{"spoiler":false}`, a name or a description of `null`) from being written to a row that has none.
fn changes_nothing(op: Op, now: &Details) -> bool {
    match op {
        Op::Name(name) => now.display_name.as_deref() == Some(name),
        Op::Describe(text) => now.description.as_deref() == Some(text),
        Op::Hide => now.spoiler,
        Op::Show => !now.spoiler,
        Op::ClearName => now.display_name.is_none(),
        Op::ClearDescription => now.description.is_none(),
    }
}

/// One device's edit, as `set_attachment_details` does it: nothing when the value is already there, otherwise rewrite the group when it
/// is there and add it when it is not. `None` is "wrote nothing".
fn apply(row: &(String, String), op: Op, at: u64) -> Option<(String, String)> {
    if changes_nothing(op, &details(row)) {
        return None;
    }
    let (group, value) = field(op);
    let present = serde_json::from_str::<Map<String, serde_json::Value>>(&row.1)
        .unwrap()
        .contains_key(group);
    let done = if present {
        let (key, field_value) = value.as_object().unwrap().iter().next().unwrap();
        rewrite_field(
            &groups_of(row),
            group,
            &hlc(at),
            None,
            (key, field_value.clone()),
        )
    } else {
        add_group(&groups_of(row), "attachment", group, &value, &hlc(at), None)
    }
    .unwrap();
    Some((done.groups_raw, done.hlc_json))
}

fn details(row: &(String, String)) -> Details {
    read_details(AttachmentKind::Image, "IMG_0001.jpg", &row.0, &row.1)
}

fn sequences() -> Vec<Vec<Op>> {
    let mut all = vec![Vec::new()];
    for first in OPS {
        all.push(vec![first]);
        for second in OPS {
            all.push(vec![first, second]);
        }
    }
    all
}

/// What the last writer of each field leaves: the op with the highest HLC per group, over both devices.
fn expected(edits: &[(Op, u64)]) -> Details {
    let last = |group: &str| {
        edits
            .iter()
            .filter(|(op, _)| field(*op).0 == group)
            .max_by_key(|(_, at)| *at)
            .map(|(op, _)| *op)
    };
    Details {
        display_name: match last("name") {
            Some(Op::Name(name)) => Some(name.to_string()),
            _ => None,
        },
        description: match last("description") {
            Some(Op::Describe(text)) => Some(text.to_string()),
            _ => None,
        },
        spoiler: matches!(last("privacy"), Some(Op::Hide)),
    }
}

#[test]
fn details_edits_from_two_devices_converge_in_every_merge_order() {
    let base = legacy();
    for (a_clock, b_clock) in [(1_000u64, 2_000u64), (2_000, 1_000)] {
        for a_ops in sequences() {
            for b_ops in sequences() {
                let (mut a, mut b) = (base.clone(), base.clone());
                let mut edits = Vec::new();
                for (i, op) in a_ops.iter().enumerate() {
                    let at = 10_000 + a_clock * (i as u64 + 1) * 2;
                    if let Some(next) = apply(&a, *op, at) {
                        a = next;
                        edits.push((*op, at));
                    }
                }
                for (i, op) in b_ops.iter().enumerate() {
                    let at = 10_000 + b_clock * (i as u64 + 1) * 2 + 1;
                    if let Some(next) = apply(&b, *op, at) {
                        b = next;
                        edits.push((*op, at));
                    }
                }
                let (va, vb) = (
                    Versioned::from_columns(&a.0, &a.1).unwrap(),
                    Versioned::from_columns(&b.0, &b.1).unwrap(),
                );
                let ab = merge(&va, &vb).record;
                let ba = merge(&vb, &va).record;
                assert_eq!(ab, ba, "{a_ops:?} against {b_ops:?}");
                let merged = ab.to_columns().unwrap();
                assert_eq!(
                    details(&merged),
                    expected(&edits),
                    "{a_ops:?} against {b_ops:?}: a rename keeps the spoiler, a hide keeps the name and the text"
                );
                // The unknown group survives every merge.
                assert!(merged.0.contains("zz_future"));
            }
        }
    }
}
