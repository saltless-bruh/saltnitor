use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Sparkline, Clear, Paragraph, BarChart},
    Frame,
};
use crate::app::App;

pub fn draw(f: &mut Frame, app: &mut App) {
    // --- NEW: Minimum height lowered to 16 since we don't stack popups ---
    if f.area().width < 80 || f.area().height < 16 {
        let warning = Paragraph::new("\n\n[!] TERMINAL FOOTPRINT TOO SMALL [!]\n\nPlease expand window to at least 80x16.")
            .alignment(ratatui::layout::Alignment::Center)
            .style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD | Modifier::RAPID_BLINK))
            .block(Block::default().borders(Borders::ALL).style(Style::default().fg(Color::Red)));
        f.render_widget(warning, f.area());
        return; 
    }
    
    // 1. Dynamic Master Layout
    let top_height = if app.show_gpu_inspector || app.show_sys_inspector { 7 } else { 6 };
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(top_height), // Dynamic Telemetry area
            Constraint::Min(0),             // Logs expand to fill middle
            Constraint::Length(5),          // Dynamic Bottom Deck
            Constraint::Length(3),          // Hotkeys
        ])
        .split(f.area());

    // 2. Dynamic Top Deck Rendering
    if app.show_gpu_inspector {
        let chunks = Layout::default().direction(Direction::Horizontal).constraints([Constraint::Percentage(55), Constraint::Percentage(45)]).split(main_chunks[0]);
        let left_block = Block::default().title(format!(" {} Architecture ", app.gpu_name)).borders(Borders::ALL).style(Style::default().fg(Color::Green));
        let right_block = Block::default().title(" Active VRAM Processes ").title_bottom("[Up/Dn] Target | [x] Kill").borders(Borders::ALL).style(Style::default().fg(Color::Cyan));
        
        let left_inner = left_block.inner(chunks[0]);
        let right_inner = right_block.inner(chunks[1]);
        f.render_widget(left_block, chunks[0]);
        f.render_widget(right_block, chunks[1]);

        let l_chunks = Layout::default().direction(Direction::Vertical).constraints([Constraint::Length(2), Constraint::Length(3)]).split(left_inner);
        let temp_color = if app.gpu_temp > 80 { Color::Red } else if app.gpu_temp > 70 { Color::Yellow } else { Color::Green };
        
        let stats_text = vec![
            Line::from(vec![Span::raw(" Core Temp:  "), Span::styled(format!("{:<15}°C", app.gpu_temp), Style::default().fg(temp_color).add_modifier(Modifier::BOLD)), Span::raw(" Fan Speed:  "), Span::styled(format!("{}%", app.gpu_fan), Style::default().fg(Color::White))]),
            Line::from(vec![Span::raw(" Power Draw: "), Span::styled(format!("{:<15}", app.gpu_power), Style::default().fg(Color::Yellow)), Span::raw(" Clocks:     "), Span::styled(&app.gpu_clocks, Style::default().fg(Color::Magenta))]),
        ];
        f.render_widget(Paragraph::new(stats_text), l_chunks[0]);

        let gauge_chunks = Layout::default().direction(Direction::Vertical).constraints([Constraint::Length(1), Constraint::Length(1)]).split(l_chunks[1]);
        
        let gpu_u_val = app.gpu_util.trim().parse::<f64>().unwrap_or(0.0) / 100.0;
        // --- Use actual VRAM Capacity math, not Bandwidth utilization ---
        let vram_ratio = if app.vram_total > 0.0 { (app.vram_used / app.vram_total).clamp(0.0, 1.0) } else { 0.0 };
        let vram_percent = (vram_ratio * 100.0) as u32;

        // --- Horizontal Splits for 1-Line Gauges ---
        let g_row = Layout::default().direction(Direction::Horizontal).constraints([Constraint::Length(19), Constraint::Min(0)]).split(gauge_chunks[0]);
        f.render_widget(Paragraph::new("[GPU Core Compute] "), g_row[0]);
        let g_gauge = Gauge::default().gauge_style(Style::default().fg(Color::Cyan)).ratio(gpu_u_val.clamp(0.0, 1.0)).label(format!("{}%", app.gpu_util.trim()));
        f.render_widget(g_gauge, g_row[1]);

        let v_row = Layout::default().direction(Direction::Horizontal).constraints([Constraint::Length(19), Constraint::Min(0)]).split(gauge_chunks[1]);
        f.render_widget(Paragraph::new("[VRAM Allocation ] "), v_row[0]);
        let v_gauge = Gauge::default().gauge_style(Style::default().fg(Color::LightBlue)).ratio(vram_ratio).label(format!("{}%", vram_percent));
        f.render_widget(v_gauge, v_row[1]);

        // --- GPU List Rendering ---
        let mut items = Vec::new();
        if app.gpu_processes.is_empty() {
            items.push(ListItem::new(Line::from(Span::styled("  [No external processes dominating VRAM]", Style::default().fg(Color::DarkGray)))));
        } else {
            for (name, mem) in &app.gpu_processes {
                items.push(ListItem::new(Line::from(Span::raw(format!("  • {:<25} | {:.2} GB", name, mem)))));
            }
        }
        let list = List::new(items).highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::REVERSED).add_modifier(Modifier::BOLD));
        f.render_stateful_widget(list, right_inner, &mut app.gpu_proc_state);

    } else if app.show_sys_inspector {
        // --- FIXED: Balanced 60/40 Split ---
        let chunks = Layout::default().direction(Direction::Horizontal).constraints([Constraint::Percentage(60), Constraint::Percentage(40)]).split(main_chunks[0]);
        let left_block = Block::default().title(format!(" {} ({}-Thread) & DDR5 ", app.cpu_name, app.cpu_core_count)).borders(Borders::ALL).style(Style::default().fg(Color::Cyan));
        let right_block = Block::default().title(" Top RAM Culprits ").title_bottom("[Up/Dn] Target | [x] Kill").borders(Borders::ALL).style(Style::default().fg(Color::Magenta));
        
        let left_inner = left_block.inner(chunks[0]);
        let right_inner = right_block.inner(chunks[1]);
        f.render_widget(left_block, chunks[0]);
        f.render_widget(right_block, chunks[1]);

        let l_chunks = Layout::default().direction(Direction::Vertical).constraints([Constraint::Length(2), Constraint::Min(0)]).split(left_inner);
        let swap_color = if app.swap_used > 1.0 { Color::Red } else { Color::Green };
        let hours = app.sys_uptime / 3600;
        let mins = (app.sys_uptime % 3600) / 60;
        
        let stat_lines = Layout::default().direction(Direction::Vertical).constraints([Constraint::Length(1), Constraint::Length(1)]).split(l_chunks[0]);
        
        // Line 1: Uptime & RAM Gauge
        let l1_split = Layout::default().direction(Direction::Horizontal).constraints([Constraint::Length(18), Constraint::Length(22), Constraint::Min(0)]).split(stat_lines[0]);
        f.render_widget(Paragraph::new(Line::from(vec![Span::raw(" Uptime: "), Span::styled(format!("{:02}h {:02}m", hours, mins), Style::default().fg(Color::White).add_modifier(Modifier::BOLD))])), l1_split[0]);
        f.render_widget(Paragraph::new(format!(" [RAM] {:>6.2}/{:<4.1} GB ", app.ram_used, app.ram_total)), l1_split[1]);
        
        let ram_ratio = if app.ram_total > 0.0 { (app.ram_used / app.ram_total).clamp(0.0, 1.0) } else { 0.0 };
        let ram_gauge = Gauge::default().gauge_style(Style::default().fg(Color::Cyan)).ratio(ram_ratio).label(format!("{:.0}%", ram_ratio * 100.0));
        f.render_widget(ram_gauge, l1_split[2]);

        // Line 2: Swap & SWP Gauge
        let l2_split = Layout::default().direction(Direction::Horizontal).constraints([Constraint::Length(18), Constraint::Length(22), Constraint::Min(0)]).split(stat_lines[1]);
        f.render_widget(Paragraph::new(Line::from(vec![Span::raw(" Swap:   "), Span::styled(format!("{:>5.2} GB", app.swap_used), Style::default().fg(swap_color).add_modifier(Modifier::BOLD))])), l2_split[0]);
        f.render_widget(Paragraph::new(format!(" [SWP] {:>6.2}/{:<4.1} GB ", app.swap_used, app.swap_total)), l2_split[1]);

        let swp_ratio = if app.swap_total > 0.0 { (app.swap_used / app.swap_total).clamp(0.0, 1.0) } else { 0.0 };
        let swp_gauge = Gauge::default().gauge_style(Style::default().fg(Color::Magenta)).ratio(swp_ratio).label(format!("{:.0}%", swp_ratio * 100.0));
        f.render_widget(swp_gauge, l2_split[2]);

        let labels: Vec<String> = (0..app.cpu_cores.len()).map(|i| format!("C{}", i)).collect();
        let mut barchart_data: Vec<(&str, u64)> = Vec::new();
        for i in 0..app.cpu_cores.len() { barchart_data.push((&labels[i], app.cpu_cores[i] as u64)); }
        
        // --- FIXED: Increased bar_gap to 2 to stretch the graph evenly ---
        let cpu_barchart = BarChart::default().block(Block::default().title(Span::styled(format!("[{}-Thread Compute Load]", app.cpu_core_count), Style::default().fg(Color::Magenta))).borders(Borders::NONE)).data(&barchart_data).bar_width(3).bar_gap(2).value_style(Style::default().fg(Color::White).bg(Color::Magenta)).bar_style(Style::default().fg(Color::Magenta));
        f.render_widget(cpu_barchart, l_chunks[1]);

        // --- Right Panel: Expanded Process Width ---
        let mut items = Vec::new();
        if app.sys_processes.is_empty() {
            items.push(ListItem::new(Line::from(Span::styled("  [No processes dominating RAM]", Style::default().fg(Color::DarkGray)))));
        } else {
            for (name, mem) in &app.sys_processes {
                // Truncate name to 24 chars instead of 16 for better readability
                let clean_name = if name.len() > 24 { format!("{}...", &name[..21]) } else { name.clone() };
                items.push(ListItem::new(Line::from(Span::raw(format!("  • {:<24} | {:>5.2} GB", clean_name, mem)))));
            }
        }
        let list = List::new(items).highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::REVERSED).add_modifier(Modifier::BOLD));
        f.render_stateful_widget(list, right_inner, &mut app.sys_proc_state);

    } else {
        // --- Standard Default Telemetry ---
        let top_chunks = Layout::default().direction(Direction::Horizontal).constraints([Constraint::Percentage(50), Constraint::Percentage(50)]).split(main_chunks[0]);
        let gauge_chunks = Layout::default().direction(Direction::Vertical).constraints([Constraint::Length(3), Constraint::Length(3)]).split(top_chunks[0]);
        
        let vram_ratio = if app.vram_total > 0.0 { (app.vram_used / app.vram_total).clamp(0.0, 1.0) } else { 0.0 };
        let vram_color = if vram_ratio >= 0.95 { Color::Red } else if vram_ratio >= 0.80 { Color::Yellow } else { Color::Green };
        let vram_title_left = if app.has_nvidia { format!(" {} VRAM Saturation ", app.gpu_name) } else { " [!] NO GPU DETECTED ".to_string() };
        let vram_title_right = Line::from(format!(" {:.2} / {:.1} GB ", app.vram_used, app.vram_total)).alignment(ratatui::layout::Alignment::Right);
        let vram_gauge = Gauge::default().block(Block::default().title(vram_title_left).title(vram_title_right).borders(Borders::ALL)).gauge_style(Style::default().fg(vram_color).add_modifier(Modifier::BOLD)).ratio(vram_ratio).label("");
        f.render_widget(vram_gauge, gauge_chunks[0]);

        let ram_ratio = if app.ram_total > 0.0 { (app.ram_used / app.ram_total).clamp(0.0, 1.0) } else { 0.0 };
        let ram_title_right = Line::from(format!(" {:.2} / {:.1} GB ", app.ram_used, app.ram_total)).alignment(ratatui::layout::Alignment::Right);
        let ram_gauge = Gauge::default().block(Block::default().title(" DDR5 System RAM Spillover ").title(ram_title_right).borders(Borders::ALL)).gauge_style(Style::default().fg(Color::Cyan)).ratio(ram_ratio).label("");
        f.render_widget(ram_gauge, gauge_chunks[1]);

        let current_cpu_load = app.cpu_history.last().copied().unwrap_or(0);
        let cpu_title = format!(" {} (Current Load: {}%) ", app.cpu_name, current_cpu_load);
        let cpu_sparkline = Sparkline::default().block(Block::default().title(cpu_title).borders(Borders::ALL)).data(&app.cpu_history).style(Style::default().fg(Color::Magenta)).max(100);
        f.render_widget(cpu_sparkline, top_chunks[1]);
    }

    // 3. Intelligent Log Analyzer
    let filtered_logs: Vec<&String> = app.logs.iter().filter(|log| {
        if app.search_query.is_empty() { true } else { log.to_lowercase().contains(&app.search_query.to_lowercase()) }
    }).collect();

    let log_items: Vec<ListItem> = filtered_logs.into_iter().map(|log| {
        let style = if log.contains("OOM") || log.contains("Failed") || log.contains("error") { Style::default().fg(Color::Red).add_modifier(Modifier::BOLD) } 
        else if log.contains("Loading Model") || log.contains("Hot-swap") { Style::default().fg(Color::Yellow) } 
        else if log.contains("llama_") { Style::default().fg(Color::Blue) } 
        else { Style::default().fg(Color::Gray) };
        ListItem::new(Line::from(Span::styled(log.clone(), style)))
    }).collect();

    let port_style = if app.port_status.contains("ZOMBIE") { Style::default().fg(Color::Red).add_modifier(Modifier::BOLD).add_modifier(Modifier::RAPID_BLINK) } 
    else if app.port_status.contains("SECURE") { Style::default().fg(Color::Green) } 
    else { Style::default().fg(Color::DarkGray) };

    let logs_title = if app.is_searching {
        Line::from(vec![Span::styled(" Log Search: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)), Span::styled(format!("{}█", app.search_query), Style::default().fg(Color::Cyan).add_modifier(Modifier::RAPID_BLINK))])
    } else if !app.search_query.is_empty() {
        Line::from(Span::styled(format!(" Logs (Filtered: {}) - Press '/' to edit ", app.search_query), Style::default().fg(Color::Yellow)))
    } else {
        Line::from(Span::styled(" Intelligent Log Analyzer ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)))
    };
    let port_title = Line::from(vec![Span::raw(" [Port: "), Span::styled(&app.port_status, port_style), Span::raw("] ")]).alignment(ratatui::layout::Alignment::Right);
    let logs_list = List::new(log_items).block(Block::default().title(logs_title).title(port_title).borders(Borders::ALL)).highlight_style(Style::default().add_modifier(Modifier::REVERSED).fg(Color::Cyan)).highlight_symbol(">> ");
    f.render_stateful_widget(logs_list, main_chunks[1], &mut app.log_state); 

    // 4. DYNAMIC BOTTOM DECK (Interrogator vs Hot-Swap)
    if app.bottom_tab_mode == 0 { // --- API Interrogator ---
        let console_border_color = if app.console_focused { Color::Cyan } else { Color::DarkGray };
        let title_target = Line::from(vec![Span::raw("[Target: "), Span::styled(&app.active_model, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)), Span::raw("] ")]).alignment(ratatui::layout::Alignment::Right);
        
        let title_mode = if app.console_focused {
            Line::from(vec![Span::styled(" [INSERT MODE] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)), Span::styled("[Press 'Esc' to exit] ", Style::default().fg(Color::DarkGray))]).alignment(ratatui::layout::Alignment::Right)
        } else {
            Line::from(vec![Span::styled(" [Tab] Hot-Swap ", Style::default().fg(Color::DarkGray)), Span::styled("| [Press 'i' to focus] ", Style::default().fg(Color::DarkGray))]).alignment(ratatui::layout::Alignment::Right)
        };

        let chars: Vec<char> = app.console_input.chars().collect();
        let before: String = chars.iter().take(app.console_cursor).collect();
        let cursor_char: String = chars.iter().skip(app.console_cursor).take(1).collect();
        let after: String = chars.iter().skip(app.console_cursor + 1).collect();

        let mut input_line = vec![Span::styled("> ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)), Span::raw(before)];
        if app.console_focused {
            if cursor_char.is_empty() { input_line.push(Span::styled(" ", Style::default().add_modifier(Modifier::REVERSED))); } 
            else { input_line.push(Span::styled(cursor_char, Style::default().add_modifier(Modifier::REVERSED))); }
            input_line.push(Span::raw(after));
        } else {
            input_line.push(Span::raw(cursor_char)); input_line.push(Span::raw(after));
        }

        let console_text = vec![
            Line::from(input_line), 
            Line::from(vec![Span::styled(format!("[TTFT: {}ms] ", app.last_ttft), Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)), Span::styled(format!("[Eval: {:.1} t/s | Gen: {:.1} t/s] ", app.last_eval_tps, app.last_gen_tps), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)), Span::styled(&app.last_api_result, Style::default().fg(Color::Gray))]),
        ];

        let console_block = Paragraph::new(console_text).block(Block::default().title(" API Interrogator ").title(title_target).title_bottom(title_mode).borders(Borders::ALL).style(Style::default().fg(console_border_color))).wrap(ratatui::widgets::Wrap { trim: true });
        f.render_widget(console_block, main_chunks[2]);
        
    } else { // --- Auto-Tuning Hot-Swap ---
        let mut items = Vec::new();
        let selected_idx = app.hot_swap_state.selected();

        for (i, model) in app.available_models.iter().enumerate() {
            let mut line_spans = vec![Span::raw(format!("  {:<50} ", model))];
            
            // Inject VRAM Oracle directly inline for the highlighted model!
            if Some(i) == selected_idx && app.console_focused {
                if let Some(est) = estimate_vram(model, app.current_ctx) {
                    if app.vram_total > 0.0 && est > app.vram_total {
                        line_spans.push(Span::styled(format!("[Oracle: {:.1} GB / {:.1} GB - HYBRID]", est, app.vram_total), Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)));
                    } else {
                        line_spans.push(Span::styled(format!("[Oracle: {:.1} GB / {:.1} GB - SAFE]", est, app.vram_total), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)));
                    }
                }
            }
            items.push(ListItem::new(Line::from(line_spans)));
        }

        let title_target = Line::from(vec![Span::raw("[Target: "), Span::styled(&app.active_model, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)), Span::raw("] ")]).alignment(ratatui::layout::Alignment::Right);
        
        let title_mode = if app.console_focused {
            Line::from(vec![Span::styled(" [Enter] Swap ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)), Span::styled("| [Esc] Cancel Focus ", Style::default().fg(Color::DarkGray))]).alignment(ratatui::layout::Alignment::Right)
        } else {
            Line::from(vec![Span::styled(" [Tab] Interrogator ", Style::default().fg(Color::DarkGray)), Span::styled("| [Press 'i' to focus] ", Style::default().fg(Color::DarkGray))]).alignment(ratatui::layout::Alignment::Right)
        };

        let border_color = if app.console_focused { Color::Cyan } else { Color::DarkGray };
        let list = List::new(items)
            .block(Block::default().title(" Auto-Tuning Hot-Swap ").title(title_target).title_bottom(title_mode).borders(Borders::ALL).style(Style::default().fg(border_color)))
            .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD).add_modifier(Modifier::REVERSED))
            .highlight_symbol("> ");
        f.render_stateful_widget(list, main_chunks[2], &mut app.hot_swap_state);
    }

    // 5. Hotkeys
    let hotkeys = Line::from(vec![
        Span::styled(" [q] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)), Span::raw("Quit  |"),
        Span::styled(" [h] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)), Span::raw("Help Menu  |"),
        Span::styled(" [t] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)), Span::raw("Deep Tuner  |"),
        Span::styled(" [Esc] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)), Span::raw("Close / Unfocus"),
    ]);
    let hotkeys_block = Paragraph::new(hotkeys).block(Block::default().borders(Borders::ALL).style(Style::default().fg(Color::DarkGray))).alignment(ratatui::layout::Alignment::Center);
    f.render_widget(hotkeys_block, main_chunks[3]);

    // 6. Config Tuner Popup
    if app.show_tuner {
        let area = centered_rect_absolute(60, 20, f.area());
        f.render_widget(Clear, area);

        let page_title = match app.tuner_page {
            0 => "COMPUTE & MEMORY",
            1 => "CONTEXT & SPECULATION",
            _ => "ORCHESTRATION & SECURITY",
        };

        let mut list_items = vec![
            Line::from(""),
            Line::from(vec![
                Span::styled(" --- [ PAGE ", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
                Span::styled(format!("{}: {} ] --- ", app.tuner_page + 1, page_title), Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
            ]).alignment(ratatui::layout::Alignment::Center),
            Line::from(""),
        ];

        let cache_types = ["f16", "f32", "bf16", "q8_0", "q4_0", "q4_1", "iq4_nl", "q5_0", "q5_1"];
        
        let items: Vec<(&str, String)> = match app.tuner_page {
            0 => vec![
                ("GPU Layers (ngl)", app.current_ngl.to_string()),
                ("Context Size (ctx)", app.current_ctx.to_string()),
                ("CPU Threads (threads)", app.current_threads.to_string()),
                ("Batch Size (n_batch)", app.current_batch.to_string()),
                ("Parallel Slots (np)", app.current_parallel.to_string()),
                ("Flash Attention", if app.flash_attn { "ON".to_string() } else { "OFF".to_string() }),
                ("Memory Lock (mlock)", if app.mlock { "ON".to_string() } else { "OFF".to_string() }),
                ("No Mem Map (no_mmap)", if app.no_mmap { "ON".to_string() } else { "OFF".to_string() }),
                ("KV Cache (K-Type)", cache_types[app.cache_k_idx].to_string()),
                ("KV Cache (V-Type)", cache_types[app.cache_v_idx].to_string()),
            ],
            1 => vec![
                ("RoPE Base", app.rope_base.to_string()),
                ("RoPE Scale", format!("{:.1}", app.rope_scale)),
                ("Defrag Threshold", format!("{:.1}", app.defrag_thold)),
                ("Draft Max", app.draft_max.to_string()),
                ("Draft Min", app.draft_min.to_string()),
            ],
            _ => vec![
                ("Threads per Batch", app.threads_batch.to_string()),
                ("U-Batch Size", app.ubatch_size.to_string()),
                ("Continuous Batching", if app.cont_batching { "ON".to_string() } else { "OFF".to_string() }),
                ("Context Shift", if app.ctx_shift { "ON".to_string() } else { "OFF".to_string() }),
                ("Metrics", if app.metrics { "ON".to_string() } else { "OFF".to_string() }),
                ("API Key", if app.api_key { "ON".to_string() } else { "OFF".to_string() }),
            ],
        };

        for (i, (label, val)) in items.iter().enumerate() {
            let style = if i == app.tuner_selected {
                Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Gray)
            };
            
            // Format layout so the values perfectly align like in your screenshot
            let pad = 24usize.saturating_sub(label.len());
            let pad_str = " ".repeat(pad);
            let val_pad = 10usize.saturating_sub(val.len());
            let val_pad_str = " ".repeat(val_pad);

            let line_str = format!("  {}:{}{}{}[< / >]  ", label, pad_str, val, val_pad_str);
            list_items.push(Line::from(Span::styled(line_str, style)).alignment(ratatui::layout::Alignment::Center));
        }

        let bottom_text = Line::from(vec![
            Span::raw("[TAB] Next Page    |    [ENTER] Apply    |    [ESC] Cancel"),
        ]).alignment(ratatui::layout::Alignment::Center);

        let tuner_block = Paragraph::new(list_items)
            .block(Block::default()
                .title(format!(" Deep router.env Tuner [Page {}/3] ", app.tuner_page + 1))
                .title_bottom(bottom_text)
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::Cyan)))
            .alignment(ratatui::layout::Alignment::Center);
            
        f.render_widget(tuner_block, area);
    }

    // 7. Interactive Help Overlay
    if app.show_help {
        let area = centered_rect_absolute(65, 22, f.area());
        f.render_widget(Clear, area);

        let help_text = vec![
            Line::from(""),
            Line::from(Span::styled(" --- Navigation & Display ---", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
            Line::from("  [q] Quit        | [PgUp/PgDn] Scroll Logs"),
            Line::from("  [/] Search Logs | [Enter] Apply Search/Filter"),
            Line::from("  [g] GPU Metrics | [c] CPU & RAM Metrics"),
            Line::from(""),
            Line::from(Span::styled(" --- Orchestration & Tuning ---", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
            Line::from("  [Tab] Toggle Bottom Deck (Interrogator vs Hot-Swap)"), // <-- UPDATED
            Line::from("  [t] Config Tuner (ngl, ctx, threads, batch, parallel)"),
            Line::from("  [i] Focus Active Deck (Press Esc to cancel)"),         // <-- UPDATED
            Line::from("  [Up/Dn] Cycle Logs / Payloads / Sniper Targets"),      // <-- UPDATED
            Line::from(""),
            Line::from(Span::styled(" --- Daemon Control (Requires Sudo) ---", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
            Line::from("  [Shift+S] Start Daemon   | [Shift+X] Stop Daemon"),
            Line::from("  [Shift+R] Restart Daemon"),
            Line::from(""),
            Line::from(Span::styled(" --- Tactical Response ---", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))),
            Line::from("  [Ctrl+D] Crash Dump (Snapshot telemetry to file)"),
            Line::from("  [Ctrl+K] Kill-Switch (Force-kill llama-server)"),
            Line::from(""),
        ];

        let popup_block = Paragraph::new(help_text)
            .block(Block::default().title(" Saltnitor Command Manual ").title_bottom(Line::from(Span::styled(" Press [h] or [Esc] to close ", Style::default().fg(Color::DarkGray))).alignment(ratatui::layout::Alignment::Right)).borders(Borders::ALL).style(Style::default().fg(Color::Yellow)))
            .alignment(ratatui::layout::Alignment::Left);
        f.render_widget(popup_block, area);
    }
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

/// The VRAM Oracle Math Engine (Heuristic Estimator)
fn estimate_vram(model: &str, ctx: i32) -> Option<f64> {
    let model_upper = model.to_uppercase();
    let mut params = 0.0;
    let mut param_found = false;

    // Parse parameter count (e.g., "30B", "8B", "70B")
    for word in model_upper.replace("-", " ").replace("_", " ").split_whitespace() {
        if word.ends_with("B") {
            if let Ok(p) = word.trim_end_matches('B').parse::<f64>() {
                params = p;
                param_found = true;
                break;
            }
        }
    }
    
    if !param_found { return None; }

    // Parse quantization bits
    let mut bits = 16.0; // Default to fp16
    if model_upper.contains("Q2") { bits = 2.5; }
    else if model_upper.contains("Q3") { bits = 3.5; }
    else if model_upper.contains("Q4") { bits = 4.5; }
    else if model_upper.contains("Q5") { bits = 5.5; }
    else if model_upper.contains("Q6") { bits = 6.5; }
    else if model_upper.contains("Q8") { bits = 8.5; }

    let file_vram = params * (bits / 8.0);
    let ctx_vram = (ctx as f64 / 1024.0) * 0.125; // Base context overhead
    Some(file_vram + ctx_vram)
}