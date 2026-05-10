use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Sparkline, Clear, Paragraph, BarChart},
    Frame,
};
use crate::app::App;

/// The main rendering function called on every frame.
pub fn draw(f: &mut Frame, app: &mut App) {
    // 1. Define the Master Layout Layout
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(10), // Telemetry area
            Constraint::Min(0),     // Log area expands to fill the middle
            Constraint::Length(4),  // API Interrogator area
            Constraint::Length(3),  // Hotkeys panel at the very bottom
        ])
        .split(f.area());

    // Split the Top section: Left side for Memory Gauges, Right side for CPU Sparkline
    let top_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(main_chunks[0]);

    // Split the Left Memory section into two stacked rows plus an audit line
    let gauge_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // VRAM Gauge height
            Constraint::Length(3), // RAM Gauge height
            Constraint::Length(1), // Spacer padding
            Constraint::Length(1), // Port Auditor line
            Constraint::Min(0),
        ])
        .split(top_chunks[0]);
    
    // 2. RTX 3060 VRAM Gauge
    let vram_ratio = if app.vram_total > 0.0 {
        (app.vram_used / app.vram_total).clamp(0.0, 1.0)
    } else {
        0.0
    };
    
    // Color coding logic: Yellow at 80%, Red at 95%
    let vram_color = if vram_ratio >= 0.95 {
        Color::Red
    } else if vram_ratio >= 0.80 {
        Color::Yellow
    } else {
        Color::Green
    };

    let vram_title = if app.has_nvidia { format!(" {} VRAM Saturation ", app.gpu_name) } else { " [!] NO COMPATIBLE GPU DETECTED ".to_string() };
    let vram_gauge = Gauge::default()
        .block(Block::default().title(vram_title).borders(Borders::ALL))
        .gauge_style(Style::default().fg(vram_color).add_modifier(Modifier::BOLD))
        .ratio(vram_ratio)
        .label(format!("{:.2} / {:.1} GB", app.vram_used, app.vram_total));
    f.render_widget(vram_gauge, gauge_chunks[0]);

    // 3. DDR5 RAM Spillover Gauge
    let ram_ratio = if app.ram_total > 0.0 {
        (app.ram_used / app.ram_total).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let ram_gauge = Gauge::default()
        .block(Block::default().title(" DDR5 System RAM Spillover ").borders(Borders::ALL))
        .gauge_style(Style::default().fg(Color::Cyan))
        .ratio(ram_ratio)
        .label(format!("{:.2} / {:.1} GB", app.ram_used, app.ram_total));
    f.render_widget(ram_gauge, gauge_chunks[1]);
    
    // 3.5 Session & Port Auditor
    let port_style = if app.port_status.contains("ZOMBIE") {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD).add_modifier(Modifier::RAPID_BLINK)
    } else if app.port_status.contains("SECURE") {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let port_alert = Paragraph::new(Span::styled(&app.port_status, port_style));
    f.render_widget(port_alert, gauge_chunks[3]);

    // 4. Ryzen 7 7700 CPU Sparkline
    let cpu_sparkline = Sparkline::default()
        .block(Block::default().title(format!(" {} Engine Load (60s) ", app.cpu_name)).borders(Borders::ALL))
        .data(&app.cpu_history)
        .style(Style::default().fg(Color::Magenta))
        .max(100);
    f.render_widget(cpu_sparkline, top_chunks[1]);

    // 5. Intelligent Log Analyzer (List Widget)
    let log_items: Vec<ListItem> = app
        .logs
        .iter()
        .map(|log| {
            let style = if log.contains("OOM") || log.contains("Failed") || log.contains("error") {
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
            } else if log.contains("Loading Model") || log.contains("Hot-swap") {
                Style::default().fg(Color::Yellow)
            } else if log.contains("llama_") {
                Style::default().fg(Color::Blue)
            } else {
                Style::default().fg(Color::Gray)
            };
            ListItem::new(Line::from(Span::styled(log.clone(), style)))
        })
        .collect();

    // Dynamically insert the CLI service name into the log title
    let logs_title = format!(" {} systemd logs [PageUp/PageDown to scroll] ", app.service_name);
    let logs_list = List::new(log_items)
        .block(Block::default().title(logs_title).borders(Borders::ALL))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED).fg(Color::Cyan))
        .highlight_symbol(">> ");
    
    f.render_stateful_widget(logs_list, main_chunks[1], &mut app.log_state);

    // 6. API Interrogator (Mini-Console)
    let console_border_color = if app.console_focused { Color::Cyan } else { Color::DarkGray };
    let console_title = if app.console_focused { " API Interrogator [INSERT MODE] " } else { " API Interrogator [Press 'i' to focus] " };

    let console_text = vec![
        Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(&app.console_input),
        ]),
        Line::from(vec![
            Span::styled(format!("[TTFT: {}ms] ", app.last_ttft), Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
            Span::styled(&app.last_api_result, Style::default().fg(Color::Gray)),
        ]),
    ];

    let console_block = Paragraph::new(console_text)
        .block(Block::default().title(console_title).borders(Borders::ALL).style(Style::default().fg(console_border_color)));
    
    f.render_widget(console_block, main_chunks[2]);

    // 6.5 Command Center Hotkeys Legend
    let hotkeys_text = Line::from(vec![
        Span::styled("[q] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw("Quit  |  "),
        Span::styled("[t] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw("Tuner  |  "),
        Span::styled("[m] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw("Models  |  "),
        Span::styled("[i] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw("API Mode  |  "),
        Span::styled("[PgUp/Dn] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw("Scroll  |  "),
        Span::styled("[g/c] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw("Hardware Inspectors  |  "),
        Span::styled("[Shift+S/X/R] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw("Daemon Start/Stop/Restart  |  "),
        Span::styled("[Ctrl+K] ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        Span::raw("Kill-Switch"),
    ]);

    let hotkeys_block = Paragraph::new(hotkeys_text)
        .block(Block::default().title(" Quick Reference ").borders(Borders::ALL).style(Style::default().fg(Color::DarkGray)))
        .alignment(ratatui::layout::Alignment::Center);
    
    f.render_widget(hotkeys_block, main_chunks[3]);

    // 7. Dynamic Config Tuner (Popup Overlay)
    if app.show_tuner {
        let area = centered_rect(40, 30, f.area());
        f.render_widget(Clear, area); 

        let ngl_style = if app.tuner_selected == 0 { Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::Gray) };
        let ctx_style = if app.tuner_selected == 1 { Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::Gray) };

        let text = vec![
            Line::from(Span::styled(format!("> GPU Layers (ngl): {}  [< / >]", app.current_ngl), ngl_style)),
            Line::from(""),
            Line::from(Span::styled(format!("> Context Size (ctx): {}  [< / >]", app.current_ctx), ctx_style)),
            Line::from(""),
            Line::from(Span::styled("[ENTER] Save & Apply  |  [ESC] Cancel", Style::default().fg(Color::DarkGray))),
        ];

        let popup_block = Paragraph::new(text)
            .block(Block::default().title(" router.ini Tuner ").borders(Borders::ALL).style(Style::default().fg(Color::Cyan)));
        
        f.render_widget(popup_block, area);
    }

    // 8. Live Model Selector (Popup Overlay)
    if app.show_model_selector {
        let area = centered_rect(50, 40, f.area());
        f.render_widget(Clear, area);

        let mut items = Vec::new();
        if app.available_models.is_empty() {
            items.push(ListItem::new("Scanning for models..."));
        } else {
            for (i, model) in app.available_models.iter().enumerate() {
                let style = if i == app.model_selector_index {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD).add_modifier(Modifier::REVERSED)
                } else {
                    Style::default().fg(Color::White)
                };
                items.push(ListItem::new(Span::styled(format!("  {}  ", model), style)));
            }
        }

        let list = List::new(items)
            .block(Block::default().title(" Target Model Selector ").borders(Borders::ALL).style(Style::default().fg(Color::Green)));
        
        f.render_widget(list, area);
    }

    // 9. GPU Inspector Popup (Vertical Stack UI)
    if app.show_gpu_inspector {
        let area = centered_rect_absolute(55, 20, f.area());
        f.render_widget(Clear, area);

        let popup_block = Block::default().title(format!(" {} Architecture ", app.gpu_name)).borders(Borders::ALL).style(Style::default().fg(Color::Green));
        let inner_area = popup_block.inner(area);
        f.render_widget(popup_block, area);

        let chunks = Layout::default().direction(Direction::Vertical).constraints([
            Constraint::Length(3), // Text Stats
            Constraint::Length(5), // Gauges
            Constraint::Min(0),    // Process List
        ]).split(inner_area);

        let temp_color = if app.gpu_temp > 80 { Color::Red } else if app.gpu_temp > 70 { Color::Yellow } else { Color::Green };
        
        let stats_text = vec![
            Line::from(vec![
                Span::raw(" Core Temp:  "), Span::styled(format!("{:<15}°C", app.gpu_temp), Style::default().fg(temp_color).add_modifier(Modifier::BOLD)),
                Span::raw(" Fan Speed:  "), Span::styled(format!("{}%", app.gpu_fan), Style::default().fg(Color::White)),
            ]),
            Line::from(vec![
                Span::raw(" Power Draw: "), Span::styled(format!("{:<15}", app.gpu_power), Style::default().fg(Color::Yellow)),
                Span::raw(" Clocks:     "), Span::styled(&app.gpu_clocks, Style::default().fg(Color::Magenta)),
            ]),
        ];
        f.render_widget(Paragraph::new(stats_text), chunks[0]);

        let gauge_chunks = Layout::default().direction(Direction::Vertical).constraints([Constraint::Length(2), Constraint::Length(2)]).split(chunks[1]);
        
        let gpu_u_val = app.gpu_util.trim().parse::<f64>().unwrap_or(0.0) / 100.0;
        let vram_u_val = app.vram_util.trim().parse::<f64>().unwrap_or(0.0) / 100.0;

        let g_gauge = Gauge::default().block(Block::default().title("GPU Core Compute").borders(Borders::NONE)).gauge_style(Style::default().fg(Color::Cyan)).ratio(gpu_u_val.clamp(0.0, 1.0)).label(format!("{}%", app.gpu_util));
        let v_gauge = Gauge::default().block(Block::default().title("VRAM Allocation").borders(Borders::NONE)).gauge_style(Style::default().fg(Color::LightBlue)).ratio(vram_u_val.clamp(0.0, 1.0)).label(format!("{}%", app.vram_util));
        
        f.render_widget(g_gauge, gauge_chunks[0]);
        f.render_widget(v_gauge, gauge_chunks[1]);

        let mut proc_text = vec![
            Line::from(""), // Spacer
            Line::from(Span::styled(" --- Active VRAM Processes ---", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)))
        ];
        if app.gpu_processes.is_empty() {
            proc_text.push(Line::from(Span::styled("  [No external processes dominating VRAM]", Style::default().fg(Color::DarkGray))));
        } else {
            for (name, mem) in &app.gpu_processes {
                proc_text.push(Line::from(format!("  • {:<25} | {:.2} GB", name, mem)));
            }
        }
        f.render_widget(Paragraph::new(proc_text), chunks[2]);
    }

    // 10. System/CPU Inspector Popup (Vertical Stack UI)
    if app.show_sys_inspector {
        let area = centered_rect_absolute(65, 24, f.area()); 
        f.render_widget(Clear, area);

        let popup_block = Block::default().title(format!(" {} & DDR5 ", app.cpu_name)).borders(Borders::ALL).style(Style::default().fg(Color::Cyan));
        let inner_area = popup_block.inner(area);
        f.render_widget(popup_block, area);

        let chunks = Layout::default().direction(Direction::Vertical).constraints([
            Constraint::Length(3),  // Uptime & Swap
            Constraint::Length(7),  // Bar Chart
            Constraint::Min(0),     // Process List
        ]).split(inner_area);

        let swap_color = if app.swap_used > 1.0 { Color::Red } else { Color::Green };
        let hours = app.sys_uptime / 3600;
        let mins = (app.sys_uptime % 3600) / 60;
        
        let stats_text = vec![
            Line::from(vec![Span::raw(" System Uptime: "), Span::styled(format!("{:02}h {:02}m", hours, mins), Style::default().fg(Color::White).add_modifier(Modifier::BOLD))]),
            Line::from(vec![Span::raw(" SSD Swap Mem:  "), Span::styled(format!("{:.2} / {:.2} GB", app.swap_used, app.swap_total), Style::default().fg(swap_color).add_modifier(Modifier::BOLD))]),
        ];
        f.render_widget(Paragraph::new(stats_text), chunks[0]);

        let labels: Vec<String> = (0..app.cpu_cores.len()).map(|i| format!("C{}", i)).collect();
        let mut barchart_data: Vec<(&str, u64)> = Vec::new();
        
        for i in 0..app.cpu_cores.len() {
            barchart_data.push((&labels[i], app.cpu_cores[i] as u64));
        }

        let chart_title = format!(" --- {}-Thread Processor Load --- ", app.cpu_core_count);
        let cpu_barchart = BarChart::default()
            .block(Block::default().title(Span::styled(chart_title, Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD))).borders(Borders::NONE))
            .data(&barchart_data)
            .bar_width(3)
            .bar_gap(1)
            .value_style(Style::default().fg(Color::Black).bg(Color::Magenta))
            .bar_style(Style::default().fg(Color::Magenta));
        
        f.render_widget(cpu_barchart, chunks[1]);

        let mut proc_text = vec![
            Line::from(""), // Spacer
            Line::from(Span::styled(" --- Top 8 System RAM Culprits ---", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)))
        ];
        for (name, mem) in &app.sys_processes {
            proc_text.push(Line::from(format!("  • {:<25} | {:.2} GB", name, mem)));
        }
        f.render_widget(Paragraph::new(proc_text), chunks[2]);
    }
}

/// Helper to construct a centered rectangle with a percentage width/height
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

/// Helper to construct a perfectly centered rectangle with absolute width and height
fn centered_rect_absolute(fixed_x: u16, fixed_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(r.height.saturating_sub(fixed_y) / 2),
            Constraint::Length(fixed_y),
            Constraint::Min(0),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(r.width.saturating_sub(fixed_x) / 2),
            Constraint::Length(fixed_x),
            Constraint::Min(0),
        ])
        .split(popup_layout[1])[1]
}