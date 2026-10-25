//! Dual-mode line breaking (Fast Greedy & Knuth-Plass optimal break) with full text alignment (`ADR-0020`, `AUTHOR-004`).

use vocument_flow::styles::Alignment;

use crate::geometry::{FixedPt, Rect};
use crate::shaper::ShapedRun;
use crate::tree::{LayoutLine, PositionedRun};

/// Configuration parameters for paragraph line breaking.
#[derive(Debug, Clone)]
pub struct LineBreakerConfig {
    pub available_width_pt: f32,
    pub alignment: Alignment,
    pub first_line_indent_pt: f32,
    pub line_spacing_mult: f32,
    pub use_knuth_plass: bool,
}

/// Breaks a sequence of shaped text runs into positioned layout lines.
pub fn break_paragraph_runs(
    runs: &[ShapedRun],
    config: &LineBreakerConfig,
    y_start_pt: f32,
) -> Vec<LayoutLine> {
    if runs.is_empty() {
        return Vec::new();
    }

    if config.use_knuth_plass {
        break_knuth_plass(runs, config, y_start_pt)
    } else {
        break_greedy(runs, config, y_start_pt)
    }
}

/// Fast greedy line-breaking algorithm for real-time interactive typing (< 16ms budget).
pub fn break_greedy(
    runs: &[ShapedRun],
    config: &LineBreakerConfig,
    y_start_pt: f32,
) -> Vec<LayoutLine> {
    let mut lines = Vec::new();
    let mut line_runs: Vec<&ShapedRun> = Vec::new();
    let mut current_line_width = FixedPt::ZERO;
    let mut is_first_line = true;
    let mut current_y = y_start_pt;

    for run in runs {
        let line_limit = if is_first_line {
            FixedPt::from_f32(config.available_width_pt - config.first_line_indent_pt)
        } else {
            FixedPt::from_f32(config.available_width_pt)
        };

        // If run is whitespace at the start of a line, skip leading whitespace
        if line_runs.is_empty() && run.is_whitespace {
            continue;
        }

        let run_adv = run.total_advance;

        if !line_runs.is_empty()
            && (current_line_width + run_adv > line_limit)
            && !run.is_whitespace
        {
            // Trim trailing whitespace from the line before assembling
            while let Some(last) = line_runs.last() {
                if last.is_whitespace {
                    current_line_width -= last.total_advance;
                    line_runs.pop();
                } else {
                    break;
                }
            }

            let layout_line = assemble_line(&line_runs, current_y, is_first_line, false, config);
            current_y += layout_line.ascent_pt - layout_line.descent_pt;
            lines.push(layout_line);

            line_runs.clear();
            current_line_width = FixedPt::ZERO;
            is_first_line = false;
        }

        line_runs.push(run);
        current_line_width += run_adv;
    }

    if !line_runs.is_empty() {
        // Trim trailing whitespace
        while let Some(last) = line_runs.last() {
            if last.is_whitespace {
                line_runs.pop();
            } else {
                break;
            }
        }

        if !line_runs.is_empty() {
            let layout_line = assemble_line(
                &line_runs,
                current_y,
                is_first_line,
                true, // Last line of paragraph
                config,
            );
            lines.push(layout_line);
        }
    }

    lines
}

/// Knuth-Plass optimal line-breaking algorithm minimizing total paragraph demerits.
pub fn break_knuth_plass(
    runs: &[ShapedRun],
    config: &LineBreakerConfig,
    y_start_pt: f32,
) -> Vec<LayoutLine> {
    // Filter out leading/trailing whitespace into discrete word and glue items
    let mut items: Vec<&ShapedRun> = Vec::new();
    for run in runs {
        if items.is_empty() && run.is_whitespace {
            continue;
        }
        items.push(run);
    }
    while let Some(last) = items.last() {
        if last.is_whitespace {
            items.pop();
        } else {
            break;
        }
    }

    let n = items.len();
    if n == 0 {
        return Vec::new();
    }

    // Dynamic programming: min_demerits[i] is the minimum cost to break items 0..i
    // prev_break[i] stores the index of the start of the line ending at i
    let mut min_demerits = vec![i64::MAX; n + 1];
    let mut prev_break = vec![0; n + 1];
    min_demerits[0] = 0;

    let target_width = FixedPt::from_f32(config.available_width_pt);

    for i in 0..n {
        if min_demerits[i] == i64::MAX {
            continue;
        }

        let is_first = i == 0;
        let line_limit = if is_first {
            FixedPt::from_f32(config.available_width_pt - config.first_line_indent_pt)
        } else {
            target_width
        };

        let mut line_width = FixedPt::ZERO;
        for j in i..n {
            line_width += items[j].total_advance;

            if line_width > line_limit && j > i {
                // Cannot fit more words on this line
                let excess = (line_width - line_limit).0 as i64;
                let demerits = excess.saturating_mul(excess);
                if min_demerits[i].saturating_add(demerits) < min_demerits[j + 1] {
                    min_demerits[j + 1] = min_demerits[i].saturating_add(demerits);
                    prev_break[j + 1] = i;
                }
                break;
            }

            let is_last_line = j == n - 1;
            let shortfall = (line_limit - line_width).0 as i64;
            let demerits = if is_last_line {
                0 // Last line does not penalize short length
            } else {
                shortfall.saturating_mul(shortfall)
            };

            let total_cost = min_demerits[i].saturating_add(demerits);
            if total_cost < min_demerits[j + 1] {
                min_demerits[j + 1] = total_cost;
                prev_break[j + 1] = i;
            }
        }
    }

    // Trace back breakpoints
    let mut breaks = Vec::new();
    let mut curr = n;
    while curr > 0 {
        let prev = prev_break[curr];
        breaks.push((prev, curr));
        curr = prev;
    }
    breaks.reverse();

    // Assemble lines from chosen breakpoints
    let mut lines = Vec::new();
    let mut current_y = y_start_pt;
    for (idx, &(start, end)) in breaks.iter().enumerate() {
        let is_first = idx == 0;
        let is_last = idx == breaks.len() - 1;
        let slice = &items[start..end];

        let layout_line = assemble_line(slice, current_y, is_first, is_last, config);
        current_y += layout_line.ascent_pt - layout_line.descent_pt;
        lines.push(layout_line);
    }

    lines
}

fn assemble_line(
    runs: &[&ShapedRun],
    line_top_y: f32,
    is_first_line: bool,
    is_last_line: bool,
    config: &LineBreakerConfig,
) -> LayoutLine {
    let mut max_ascent: f32 = 0.0;
    let mut max_descent: f32 = 0.0;
    let mut total_width_fixed = FixedPt::ZERO;
    let mut whitespace_count = 0;
    let mut footnote_refs = Vec::new();

    for run in runs {
        max_ascent = max_ascent.max(run.ascender_pt);
        max_descent = max_descent.min(run.descender_pt);
        total_width_fixed += run.total_advance;
        if run.is_whitespace {
            whitespace_count += 1;
        }
        if let Some(ref_id) = &run.footnote_ref {
            footnote_refs.push(ref_id.clone());
        }
    }

    if max_ascent <= 0.0 {
        max_ascent = 12.0;
    }
    if max_descent >= 0.0 {
        max_descent = -3.0;
    }

    let line_height_pt = (max_ascent - max_descent) * config.line_spacing_mult;
    let baseline_y = line_top_y + max_ascent;

    let available_width_pt = if is_first_line {
        config.available_width_pt - config.first_line_indent_pt
    } else {
        config.available_width_pt
    };

    let total_width_pt = total_width_fixed.to_f32();
    let extra_space_pt = (available_width_pt - total_width_pt).max(0.0);

    // Compute line start x offset based on alignment
    let mut start_x = if is_first_line {
        config.first_line_indent_pt
    } else {
        0.0
    };

    let space_distribution = match config.alignment {
        Alignment::Left => 0.0,
        Alignment::Center => {
            start_x += extra_space_pt / 2.0;
            0.0
        }
        Alignment::Right => {
            start_x += extra_space_pt;
            0.0
        }
        Alignment::Justified => {
            if is_last_line || whitespace_count == 0 {
                0.0 // Last line of justified paragraph aligns left
            } else {
                extra_space_pt / whitespace_count as f32
            }
        }
    };

    let mut current_x = start_x;
    let mut positioned_runs = Vec::with_capacity(runs.len());

    for run in runs {
        let run_width_pt = run.advance_pt();
        let bounds = Rect::new(current_x, line_top_y, run_width_pt, line_height_pt);

        positioned_runs.push(PositionedRun {
            text: run.text.clone(),
            bounds,
            font_family: run.font.family.clone(),
            font_size_pt: run.font_size_pt,
            font_style: run.font_style,
            color: run.color,
            link_url: run.link_url.clone(),
            footnote_ref: run.footnote_ref.clone(),
        });

        current_x += run_width_pt;
        if run.is_whitespace && config.alignment == Alignment::Justified && !is_last_line {
            current_x += space_distribution;
        }
    }

    let line_width = (current_x - start_x).max(0.0);
    let bounds = Rect::new(start_x, line_top_y, line_width, line_height_pt);

    LayoutLine {
        bounds,
        runs: positioned_runs,
        baseline_y,
        ascent_pt: max_ascent,
        descent_pt: max_descent,
        footnote_refs,
    }
}
