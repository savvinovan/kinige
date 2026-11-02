//! The forest (PRD §7), spike S3 candidate: where a day sits on the board for both candidate grids, the season of a
//! date, the weight of a day and its growth stage. No IO. The same rules (and the same golden table) are written in
//! TypeScript in `spikes/S3-forest/src/calendar.ts`, because the spike page cannot call Rust.
//!
//! Not wired into the API yet: slice 6 decides the IPC shape and adds the ts-rs types.

use chrono::{Datelike, NaiveDate};

/// Number of growth stages above zero: stage 6 is the biggest tree (PRD §7.2).
pub const MAX_STAGE: u8 = 6;
/// Default `w_max` of the stage formula (PRD §7.2).
pub const DEFAULT_W_MAX: f64 = 30.0;

/// The candidate grids of PRD §7.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForestGrid {
    /// A: 12 rows (months) x 31 columns (days); the 6 or 7 non-existent dates are empty ground.
    MonthByDay,
    /// B: 7 rows (weekdays) x 53 columns (weeks); a leap year that starts on the last day of the week needs 54.
    WeekdayByWeek,
}

/// The first day of a week in grid B.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirstWeekday {
    Monday,
    Sunday,
}

/// A cell of the board.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellPos {
    pub row: u8,
    pub col: u8,
}

/// Size of the board for a year.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridDims {
    pub rows: u8,
    pub cols: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Season {
    Winter,
    Spring,
    Summer,
    Autumn,
}

/// What the day weight needs to know about one message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoteStat {
    /// Length in characters.
    pub len: usize,
    pub attachments: usize,
}

/// The position of a day: `(year, date, first_weekday) → (row, col)`.
pub fn day_position(grid: ForestGrid, date: NaiveDate, first: FirstWeekday) -> CellPos {
    match grid {
        ForestGrid::MonthByDay => CellPos {
            row: small(date.month0()),
            col: small(date.day0()),
        },
        ForestGrid::WeekdayByWeek => {
            let idx = january_offset(date.year(), first) + date.ordinal0();
            CellPos {
                row: small(idx % 7),
                col: small(idx / 7),
            }
        }
    }
}

/// The same for a date given as numbers; `None` for a date that does not exist (30 February, month 13…).
pub fn day_position_ymd(
    grid: ForestGrid,
    year: i32,
    month: u32,
    day: u32,
    first: FirstWeekday,
) -> Option<CellPos> {
    NaiveDate::from_ymd_opt(year, month, day).map(|date| day_position(grid, date, first))
}

/// The inverse: the date shown in a cell, `None` for empty ground and for cells outside the board.
pub fn date_at_cell(
    grid: ForestGrid,
    year: i32,
    first: FirstWeekday,
    cell: CellPos,
) -> Option<NaiveDate> {
    let dims = grid_dims(grid, year, first)?;
    if cell.row >= dims.rows || cell.col >= dims.cols {
        return None;
    }
    match grid {
        ForestGrid::MonthByDay => {
            NaiveDate::from_ymd_opt(year, u32::from(cell.row) + 1, u32::from(cell.col) + 1)
        }
        ForestGrid::WeekdayByWeek => {
            let linear = u32::from(cell.col) * 7 + u32::from(cell.row);
            let ordinal0 = linear.checked_sub(january_offset(year, first))?;
            NaiveDate::from_yo_opt(year, ordinal0 + 1)
        }
    }
}

/// Size of the board: A is always 12 x 31; B is 7 x 53, or 7 x 54 when 1 January is the last day of the week of a
/// leap year (2012 and 2040 with a Monday start, 2000 and 2028 with a Sunday start). `None` for a year chrono cannot
/// represent.
pub fn grid_dims(grid: ForestGrid, year: i32, first: FirstWeekday) -> Option<GridDims> {
    match grid {
        ForestGrid::MonthByDay => Some(GridDims { rows: 12, cols: 31 }),
        ForestGrid::WeekdayByWeek => {
            let days = NaiveDate::from_ymd_opt(year, 12, 31)?.ordinal();
            let cells = january_offset(year, first) + days;
            Some(GridDims {
                rows: 7,
                cols: small(cells.div_ceil(7)),
            })
        }
    }
}

/// Season of a month (1..=12); `southern` shifts it by six months (PRD §7.3). Anything else is treated as winter.
pub fn season_of_month(month: u32, southern: bool) -> Season {
    if !(1..=12).contains(&month) {
        return Season::Winter;
    }
    let m = if southern {
        (month + 5) % 12 + 1
    } else {
        month
    };
    match m {
        3..=5 => Season::Spring,
        6..=8 => Season::Summer,
        9..=11 => Season::Autumn,
        _ => Season::Winter,
    }
}

/// `w = Σ min(1, len/200) + 0.5 × attachments`; a message shorter than 3 characters without attachments does not
/// count (PRD §7.2).
pub fn day_weight(notes: &[NoteStat]) -> f64 {
    notes
        .iter()
        .filter(|n| n.len >= 3 || n.attachments > 0)
        .map(|n| (n.len as f64 / 200.0).min(1.0) + 0.5 * n.attachments as f64)
        .sum()
}

/// `stage = clamp(round(6 × ln(1+w) / ln(1+w_max)), 0, 6)`. A negative or non-finite weight, or a `w_max` that is
/// not above zero, gives stage 0.
pub fn stage_of(weight: f64, w_max: f64) -> u8 {
    if !weight.is_finite() || !w_max.is_finite() || weight <= 0.0 || w_max <= 0.0 {
        return 0;
    }
    let raw = f64::from(MAX_STAGE) * weight.ln_1p() / w_max.ln_1p();
    // Saturating float-to-int cast after the clamp: the value is in 0.0..=6.0.
    raw.round().clamp(0.0, f64::from(MAX_STAGE)) as u8
}

/// How many empty cells sit above 1 January in the first column of grid B.
fn january_offset(year: i32, first: FirstWeekday) -> u32 {
    let Some(jan1) = NaiveDate::from_ymd_opt(year, 1, 1) else {
        return 0;
    };
    match first {
        FirstWeekday::Monday => jan1.weekday().num_days_from_monday(),
        FirstWeekday::Sunday => jan1.weekday().num_days_from_sunday(),
    }
}

/// Cell coordinates are at most 53; saturate instead of wrapping if that ever stops being true.
fn small(n: u32) -> u8 {
    u8::try_from(n).unwrap_or(u8::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: ForestGrid = ForestGrid::MonthByDay;
    const B: ForestGrid = ForestGrid::WeekdayByWeek;
    const MON: FirstWeekday = FirstWeekday::Monday;
    const SUN: FirstWeekday = FirstWeekday::Sunday;

    /// The same rows as `GOLDEN` in `spikes/S3-forest/src/calendar.test.ts`.
    #[allow(clippy::type_complexity)]
    const GOLDEN: &[(ForestGrid, FirstWeekday, i32, u32, u32, Option<(u8, u8)>)] = &[
        (A, MON, 2026, 1, 1, Some((0, 0))),
        (A, SUN, 2026, 2, 28, Some((1, 27))),
        (A, MON, 2026, 2, 29, None),
        (A, MON, 2024, 2, 29, Some((1, 28))),
        (A, MON, 2026, 4, 31, None),
        (A, MON, 2026, 12, 31, Some((11, 30))),
        (B, MON, 2026, 1, 1, Some((3, 0))),
        (B, SUN, 2026, 1, 1, Some((4, 0))),
        (B, MON, 2026, 12, 31, Some((3, 52))),
        (B, SUN, 2026, 12, 31, Some((4, 52))),
        (B, MON, 2024, 1, 1, Some((0, 0))),
        (B, MON, 2024, 2, 29, Some((3, 8))),
        (B, MON, 2024, 12, 31, Some((1, 52))),
        (B, SUN, 2024, 1, 1, Some((1, 0))),
        (B, SUN, 2024, 12, 31, Some((2, 52))),
        (B, MON, 2012, 1, 1, Some((6, 0))),
        (B, MON, 2012, 12, 31, Some((0, 53))),
        (B, SUN, 2028, 1, 1, Some((6, 0))),
        (B, SUN, 2028, 12, 31, Some((0, 53))),
        (B, MON, 2026, 2, 30, None),
    ];

    #[test]
    fn golden_table() {
        for &(grid, first, y, m, d, want) in GOLDEN {
            let got = day_position_ymd(grid, y, m, d, first).map(|c| (c.row, c.col));
            assert_eq!(got, want, "{grid:?} {first:?} {y}-{m}-{d}");
        }
    }

    #[test]
    fn grid_b_needs_a_54th_column_exactly_in_those_leap_years() {
        for (first, want) in [
            (MON, vec![2012, 2040, 2068, 2096]),
            (SUN, vec![2000, 2028, 2056, 2084]),
        ] {
            let wide: Vec<i32> = (1990..=2100)
                .filter(|&y| grid_dims(B, y, first).is_some_and(|d| d.cols == 54))
                .collect();
            assert_eq!(wide, want, "{first:?}");
        }
        assert_eq!(
            grid_dims(A, 2026, MON),
            Some(GridDims { rows: 12, cols: 31 })
        );
    }

    #[test]
    fn every_day_has_a_unique_cell_inside_the_board_and_the_inverse_returns_it() {
        for grid in [A, B] {
            for first in [MON, SUN] {
                for year in 1990..=2100 {
                    let dims = grid_dims(grid, year, first).unwrap();
                    let mut seen = std::collections::HashSet::new();
                    let mut n = 0;
                    let mut date = NaiveDate::from_ymd_opt(year, 1, 1).unwrap();
                    while date.year() == year {
                        let pos = day_position(grid, date, first);
                        assert!(
                            pos.row < dims.rows && pos.col < dims.cols,
                            "{grid:?} {date}"
                        );
                        assert!(seen.insert((pos.row, pos.col)), "{grid:?} {date} collides");
                        assert_eq!(date_at_cell(grid, year, first, pos), Some(date));
                        n += 1;
                        date = date.succ_opt().unwrap();
                    }
                    assert_eq!(
                        n,
                        if date.pred_opt().unwrap().ordinal() == 366 {
                            366
                        } else {
                            365
                        }
                    );
                }
            }
        }
    }

    #[test]
    fn grid_a_has_six_or_seven_empty_cells() {
        for (year, missing) in [(2026, 7), (2024, 6), (2100, 7), (2000, 6)] {
            let empty = (0..12u8)
                .flat_map(|row| (0..31u8).map(move |col| CellPos { row, col }))
                .filter(|&c| date_at_cell(A, year, MON, c).is_none())
                .count();
            assert_eq!(empty, missing, "{year}");
        }
    }

    #[test]
    fn garbage_input_gives_none_not_a_panic() {
        assert_eq!(day_position_ymd(A, 2026, 13, 1, MON), None);
        assert_eq!(day_position_ymd(B, 2026, 0, 1, MON), None);
        assert_eq!(day_position_ymd(B, 2026, 6, 0, SUN), None);
        assert_eq!(day_position_ymd(A, i32::MAX, 1, 1, MON), None);
        assert_eq!(grid_dims(B, i32::MIN, MON), None);
        assert_eq!(
            date_at_cell(B, i32::MAX, MON, CellPos { row: 0, col: 0 }),
            None
        );
        for cell in [
            CellPos { row: 12, col: 0 },
            CellPos { row: 0, col: 31 },
            CellPos {
                row: u8::MAX,
                col: u8::MAX,
            },
        ] {
            assert_eq!(date_at_cell(A, 2026, MON, cell), None);
        }
        for cell in [
            CellPos { row: 7, col: 0 },
            CellPos { row: 0, col: 53 },
            CellPos {
                row: u8::MAX,
                col: u8::MAX,
            },
        ] {
            assert_eq!(date_at_cell(B, 2026, MON, cell), None);
        }
        assert_eq!(date_at_cell(B, 2026, MON, CellPos { row: 0, col: 0 }), None); // before 1 January
        assert_eq!(
            date_at_cell(B, 2026, MON, CellPos { row: 6, col: 52 }),
            None
        ); // after 31 December
    }

    #[test]
    fn seasons() {
        assert_eq!(season_of_month(12, false), Season::Winter);
        assert_eq!(season_of_month(1, false), Season::Winter);
        assert_eq!(season_of_month(4, false), Season::Spring);
        assert_eq!(season_of_month(7, false), Season::Summer);
        assert_eq!(season_of_month(10, false), Season::Autumn);
        assert_eq!(season_of_month(1, true), Season::Summer);
        assert_eq!(season_of_month(7, true), Season::Winter);
        assert_eq!(season_of_month(0, false), Season::Winter);
        assert_eq!(season_of_month(99, true), Season::Winter);
    }

    #[test]
    fn weight_and_stage() {
        let note = |len, attachments| NoteStat { len, attachments };
        assert_eq!(day_weight(&[]), 0.0);
        assert_eq!(day_weight(&[note(2, 0)]), 0.0);
        assert!((day_weight(&[note(100, 0)]) - 0.5).abs() < 1e-12);
        assert_eq!(day_weight(&[note(5000, 0)]), 1.0);
        assert!((day_weight(&[note(1, 1)]) - 0.505).abs() < 1e-12);
        assert_eq!(day_weight(&[note(200, 2)]), 2.0);
        assert_eq!(stage_of(0.0, DEFAULT_W_MAX), 0);
        assert_eq!(stage_of(1.0, DEFAULT_W_MAX), 1);
        assert_eq!(stage_of(5.0, DEFAULT_W_MAX), 3);
        assert_eq!(stage_of(30.0, DEFAULT_W_MAX), 6);
        assert_eq!(stage_of(1e9, DEFAULT_W_MAX), 6);
        let mut prev = 0;
        for i in 0..=160 {
            let s = stage_of(f64::from(i) / 4.0, DEFAULT_W_MAX);
            assert!(s >= prev);
            prev = s;
        }
    }

    #[test]
    fn stage_of_garbage_is_zero() {
        for (w, max) in [
            (f64::NAN, 30.0),
            (f64::INFINITY, 30.0),
            (-1.0, 30.0),
            (5.0, 0.0),
            (5.0, -3.0),
            (5.0, f64::NAN),
            (5.0, f64::INFINITY),
        ] {
            assert_eq!(stage_of(w, max), 0, "{w} {max}");
        }
    }
}
