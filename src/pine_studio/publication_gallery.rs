//! Repository screenshots drawn by the real chart and Pine Studio views. The deterministic
//! fixture is labeled in the frame; it never uses a live account or requests market data.

use std::time::{Duration, Instant};

use crate::model::chart::price_scale::PriceScaleMode;
use crate::model::{DataSource, IndicatorConfig, Kline, Timeframe, Tool};
use crate::pine_studio::debug::studio_tests::Rig;
use crate::pine_studio::library::prefs::ChartPosition;
use crate::ui::render_tests::{Screen, gallery_dir};
use crate::ui::theme::{self, AppTheme};
use crate::ui::{CHART_HEADER_HEIGHT, ChartCard, DrawingFrame, ReplayChartFrame, kit};
use eframe::egui::{self, Rect, UiBuilder, vec2};

const SAMPLE_SCRIPT: &str = "//@version=6
indicator(\"FakingView sample trend ribbon\", overlay = true)

// Demonstration bars: not a live market-data feed.
fastLength = input.int(20, \"Fast EMA\")
slowLength = input.int(50, \"Slow EMA\")

fast = ta.ema(close, fastLength)
slow = ta.ema(close, slowLength)

plot(fast, \"Fast EMA\", color = color.orange, linewidth = 2)
plot(slow, \"Slow EMA\", color = color.teal, linewidth = 2)
";

fn sample_bars() -> Vec<Kline> {
    let start = 1_700_000_000_000;
    let hour = 3_600_000;
    let mut previous: f64 = 1_520.0;
    (0..280)
        .map(|index| {
            let x = index as f64;
            let close = ((1_520.0 + x * 0.55 + (x * 0.095).sin() * 27.0 + (x * 0.63).sin() * 6.0)
                * 100.0)
                .round()
                / 100.0;
            let open = previous;
            previous = close;
            Kline {
                open_time: start + index * hour,
                close_time: start + (index + 1) * hour - 1,
                open,
                high: ((open.max(close) + 3.0 + (x * 0.27).sin().abs() * 6.0) * 100.0).round()
                    / 100.0,
                low: ((open.min(close) - 3.0 - (x * 0.31).cos().abs() * 5.0) * 100.0).round()
                    / 100.0,
                close,
                volume: 40_000.0 + (x * 0.41).sin().abs() * 80_000.0,
            }
        })
        .collect()
}

fn sample_caption(ui: &mut egui::Ui, title: &str) {
    ui.horizontal(|ui| {
        ui.label(kit::text(
            title,
            kit::size::HEADLINE,
            kit::SEMIBOLD,
            theme::text_primary(),
        ));
        kit::status_badge(ui, "SAMPLE DATA · NOT LIVE", kit::Tone::Neutral);
    });
    ui.add_space(8.0);
}

#[test]
#[ignore = "writes labeled publication screenshots through the actual UI renderer"]
fn fakingview_publication_screenshots() {
    if gallery_dir().is_none() {
        return;
    }
    let _theme = theme::RestoreDefaultTheme;
    let bars = sample_bars();
    let mut screen = Screen::new(vec2(1_280.0, 760.0));
    theme::apply(&screen.ctx, AppTheme::Dark);
    let mut card = ChartCard::default();
    card.set_timeframe(Timeframe::HOUR_1);
    card.set_visible_candles(150);
    card.set_top_inset(CHART_HEADER_HEIGHT);
    card.set_title(Some("NSE:INFY · demonstration fixture".into()));
    let indicators = IndicatorConfig {
        show_volume: true,
        show_rsi: true,
        show_ema: true,
        show_bollinger: true,
        ..Default::default()
    };
    let mut last = None;
    for _ in 0..6 {
        last = Some(screen.run(Vec::new(), |ui| {
            sample_caption(ui, "FakingView · Chart renderer");
            let area = ui.available_rect_before_wrap();
            let mut drawing =
                DrawingFrame::new(&[], Tool::Cursor, None, 1).with_view(Timeframe::HOUR_1, false);
            let mut replay = ReplayChartFrame {
                selecting: false,
                active: false,
                picked_ms: None,
                picked_range: None,
            };
            let mut chart = ui.new_child(UiBuilder::new().max_rect(area));
            card.body(
                &mut chart,
                "NSE:INFY",
                &bars,
                false,
                None,
                PriceScaleMode::Regular,
                DataSource::Kite,
                &indicators,
                None,
                1,
                &mut drawing,
                &mut replay,
            );
            let header = Rect::from_min_size(area.min, vec2(area.width(), CHART_HEADER_HEIGHT));
            let mut header_ui = ui.new_child(UiBuilder::new().max_rect(header));
            card.header(
                &mut header_ui,
                "NSE:INFY",
                DataSource::Kite,
                &indicators,
                &bars,
                false,
                false,
                &[],
                &mut Default::default(),
                1,
            );
        }));
    }
    screen.save("fakingview-hero", last.expect("a rendered chart"));

    let mut rig = Rig::new("publication-gallery");
    rig.set_source(SAMPLE_SCRIPT);
    if let Some(open) = &mut rig.manager.open {
        open.name = "FakingView sample trend ribbon".into();
    }
    rig.manager.prefs.chart_position = ChartPosition::Left;
    rig.manager.prefs.chart_width = 660.0;
    rig.manager.prefs.drawer_open = false;
    rig.manager.preview.select_timeframe(Timeframe::HOUR_1);
    rig.manager.preview.set_candles_for_test(bars);
    rig.manager.preview.set_visible_candles_for_test(150);
    let mut studio = Screen::new(vec2(1_360.0, 860.0));
    theme::apply(&studio.ctx, AppTheme::Dark);
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut last = None;
    let mut ready_frames = 0;
    for frame in 0..300 {
        theme::sync(&studio.ctx);
        last = Some(studio.run(Vec::new(), |ui| {
            sample_caption(ui, "FakingView · Pine Studio");
            rig.manager.ui(ui, None);
        }));
        if frame > 8
            && rig.manager.preview.visual().is_some_and(|output| {
                output.overlay && output.plots.len() == 2 && output.bar_count == 280
            })
        {
            // Preview work settles at the end of a UI frame. Draw its new output again
            // before saving, so the pixels and editor source describe the same run.
            ready_frames += 1;
            if ready_frames >= 3 {
                break;
            }
        } else {
            ready_frames = 0;
        }
        assert!(
            Instant::now() < deadline,
            "sample script did not produce a preview"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        rig.manager.preview.visual().is_some_and(|output| {
            output.overlay && output.plots.len() == 2 && output.bar_count == 280
        }),
        "sample EMA source was not the rendered output"
    );
    assert!(rig.manager.preview.runtime_error().is_none());
    studio.save("fakingview-pine-studio", last.expect("a rendered studio"));
}
