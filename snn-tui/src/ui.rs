// TUI rendering: 2D spike raster (time on X, neuron IDs on Y) + status panel.

use std::io::Stdout;

use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::Text,
    widgets::{Block, Borders, Paragraph},
    Terminal,
};

use crate::app::App;
use crate::backend::SnnBackend;

/// Draws the UI each frame:
/// - Top: Spike raster grid as rows (neurons) x columns (time, circular).
/// - Bottom: Status including tick, neuron count, run state, controls.
pub fn draw<B: SnnBackend>(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    app: &App<B>,
) -> anyhow::Result<()> {
    terminal.draw(|f| {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([Constraint::Percentage(80), Constraint::Percentage(20)].as_ref())
            .split(f.size());

        // Build raster lines. Each row: "nXX |....."
        let mut lines = Vec::with_capacity(app.raster.len());
        let mut buf = String::new();
        for (row_idx, row) in app.raster.iter().enumerate() {
            buf.clear();
            // label
            if row_idx < 10 {
                buf.push_str(&format!("n0{} |", row_idx));
            } else {
                buf.push_str(&format!("n{} |", row_idx));
            }
            // content
            for &ch in row.iter() {
                buf.push(ch);
            }
            lines.push(buf.clone());
        }

        let raster_text = Text::from(lines.join("\n"));
        let raster_widget = Paragraph::new(raster_text)
            .block(Block::default().title("Spike Raster  (time →)").borders(Borders::ALL))
            .style(Style::default().fg(Color::White));
        f.render_widget(raster_widget, chunks[0]);

        // Status and controls with budgets/plasticity info
        let budgets = app.budgets.unwrap_or(snn_core_plus::StepBudgets { max_edge_visits: None, max_spikes_scheduled: None });
        let bev = budgets.max_edge_visits.map(|v| v.to_string()).unwrap_or("-".to_string());
        let bss = budgets.max_spikes_scheduled.map(|v| v.to_string()).unwrap_or("-".to_string());
        #[cfg(feature = "plasticity")]
        let plast = if app.plast_on { "on" } else { "off" };
        #[cfg(not(feature = "plasticity"))]
        let plast = "n/a";

        let status = format!(
            "Tick: {} | Neurons: {} | Running: {} | Budgets: edges={} spikes={} | Plasticity: {} | Controls: [s] Step  [r] Run/Pause  [+/-] edges  [[]/] spikes  [p] plast  [q] Quit",
            app.tick,
            app.backend.neurons(),
            if app.running { "yes" } else { "no" },
            bev, bss, plast
        );
        let status_widget = Paragraph::new(status)
            .style(Style::default().fg(Color::Cyan))
            .block(Block::default().title("Status").borders(Borders::ALL));
        f.render_widget(status_widget, chunks[1]);
    })?;
    Ok(())
}