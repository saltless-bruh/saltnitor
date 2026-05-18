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
    // --- NEW: Terminal Size Safety Check ---
    if f.area().width < 80 || f.area().height < 24 {
        let warning = Paragraph::new("\n\n[!] TERMINAL FOOTPRINT TOO SMALL [!]\n\nPlease expand window to at least 80x24.")
            .alignment(ratatui::layout::Alignment::Center)
            .style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD | Modifier::RAPID_BLINK))
            .block(Block::default().borders(Borders::ALL).style(Style::default().fg(Color::Red)));
            
        f.render_widget(warning, f.area());
        return; // Halt the rest of the UI rendering to prevent a math panic
    }
    
    // 1. Define the Master Layout Layout
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6), // Telemetry area
            Constraint::Min(0),     // Log area expands to fill the middle
            Constraint::Length(5),  // API Interrogator area
            Constraint::Length(3),
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

    // --- NEW: Split the title into left and right components ---
    let vram_title_left = if app.has_nvidia { format!(" {} VRAM Saturation ", app.gpu_name) } else { " [!] NO COMPATIBLE GPU DETECTED ".to_string() };
    let vram_title_right = Line::from(format!(" {:.2} / {:.1} GB ", app.vram_used, app.vram_total)).alignment(ratatui::layout::Alignment::Right);

    let vram_gauge = Gauge::default()
        .block(Block::default()
            .title(vram_title_left)
            .title(vram_title_right) // Adds the numbers to the right side of the border
            .borders(Borders::ALL))
        .gauge_style(Style::default().fg(vram_color).add_modifier(Modifier::BOLD))
        .ratio(vram_ratio)
        .label(""); // Clears the text out of the colored bar

    f.render_widget(vram_gauge, gauge_chunks[0]);

    // 3. DDR5 RAM Spillover Gauge
    let ram_ratio = if app.ram_total > 0.0 {
        (app.ram_used / app.ram_total).clamp(0.0, 1.0)
    } else {
        0.0
    };

    // --- NEW: Split the title into left and right components ---
    let ram_title_right = Line::from(format!(" {:.2} / {:.1} GB ", app.ram_used, app.ram_total)).alignment(ratatui::layout::Alignment::Right);

    let ram_gauge = Gauge::default()
        .block(Block::default()
            .title(" DDR5 System RAM Spillover ")
            .title(ram_title_right) // Adds the numbers to the right side of the border
            .borders(Borders::ALL))
        .gauge_style(Style::default().fg(Color::Cyan))
        .ratio(ram_ratio)
        .label(""); // Clears the text out of the colored bar

    f.render_widget(ram_gauge, gauge_chunks[1]);

    // 4. Ryzen 7 7700 CPU Sparkline
    // --- NEW: Grab the most recent load value from the history buffer ---
    let current_cpu_load = app.cpu_history.last().copied().unwrap_or(0);
    let cpu_title = format!(" {} (Current Load: {}%) ", app.cpu_name, current_cpu_load);

    let cpu_sparkline = Sparkline::default()
        .block(Block::default().title(cpu_title).borders(Borders::ALL)) // <-- Use the dynamic title
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

    // --- Define Port Status Style ---
    let port_style = if app.port_status.contains("ZOMBIE") {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD).add_modifier(Modifier::RAPID_BLINK)
    } else if app.port_status.contains("SECURE") {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let logs_title = Line::from(Span::styled(
        " Intelligent Log Analyzer ",
        Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
    ));

    let port_title = Line::from(vec![
        Span::raw(" [Port: "),
        Span::styled(&app.port_status, port_style),
        Span::raw("] "),
    ])
    .alignment(ratatui::layout::Alignment::Right);

    // --- Inject title_bottom into the Block ---
    let logs_list = List::new(log_items)
        .block(
            Block::default()
                .title(logs_title)
                .title(port_title)
                .borders(Borders::ALL)
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED).fg(Color::Cyan))
        .highlight_symbol(">> ");
    
    f.render_stateful_widget(logs_list, main_chunks[1], &mut app.log_state);

    // 6. API Interrogator (Mini-Console)
    let console_border_color = if app.console_focused { Color::Cyan } else { Color::DarkGray };

    // --- NEW: Split titles and distribute them across the borders ---
    let title_main = " API Interrogator ";

    let title_target = Line::from(vec![
        Span::raw("[Target: "),
        Span::styled(&app.active_model, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw("] "),
    ]).alignment(ratatui::layout::Alignment::Right);

    let title_mode = if app.console_focused {
        // --- NEW: Added the 'Esc to exit' hint with a subtle gray style ---
        Line::from(vec![
            Span::styled(" [INSERT MODE] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled("[Press 'Esc' to exit] ", Style::default().fg(Color::DarkGray)),
        ])
        .alignment(ratatui::layout::Alignment::Right)
    } else {
        Line::from(Span::styled(" [Press 'i' to focus] ", Style::default().fg(Color::DarkGray)))
            .alignment(ratatui::layout::Alignment::Right)
    };

    // --- NEW: Dynamic Cursor Rendering ---
    let chars: Vec<char> = app.console_input.chars().collect();
    let before: String = chars.iter().take(app.console_cursor).collect();
    let cursor_char: String = chars.iter().skip(app.console_cursor).take(1).collect();
    let after: String = chars.iter().skip(app.console_cursor + 1).collect();

    // Build the input line with a highlighted block cursor if focused
    let mut input_line = vec![
        Span::styled("> ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw(before),
    ];

    if app.console_focused {
        if cursor_char.is_empty() {
            // Cursor is at the very end of the string
            input_line.push(Span::styled(" ", Style::default().add_modifier(Modifier::REVERSED)));
        } else {
            // Cursor is highlighting a specific character
            input_line.push(Span::styled(cursor_char, Style::default().add_modifier(Modifier::REVERSED)));
        }
        input_line.push(Span::raw(after));
    } else {
        // When not focused, just print normally without the block cursor
        input_line.push(Span::raw(cursor_char));
        input_line.push(Span::raw(after));
    }

    let console_text = vec![
        Line::from(input_line), // Use the dynamically built cursor line
        Line::from(vec![
            Span::styled(format!("[TTFT: {}ms] ", app.last_ttft), Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
            // --- UPDATED: Multi-Metric Display ---
            Span::styled(format!("[Eval: {:.1} t/s | Gen: {:.1} t/s] ", app.last_eval_tps, app.last_gen_tps), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled(&app.last_api_result, Style::default().fg(Color::Gray)),
        ]),
    ];

    let console_block = Paragraph::new(console_text)
        .block(
            Block::default()
                .title(title_main)
                .title(title_target)      
                .title_bottom(title_mode) 
                .borders(Borders::ALL)
                .style(Style::default().fg(console_border_color))
        )
        .wrap(ratatui::widgets::Wrap { trim: true }); // <-- NEW: Forces text to wrap beautifully!
    
    f.render_widget(console_block, main_chunks[2]);

    // 6.5 Core Quick Reference (Restored & Minimal)
    let core_hotkeys = Line::from(vec![
        Span::styled(" [q] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)), Span::raw("Quit  |"),
        Span::styled(" [h] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)), Span::raw("Full Help Menu  |"),
        Span::styled(" [t] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)), Span::raw("Deep Tuner  |"),
        Span::styled(" [m] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)), Span::raw("Models  |"),
        Span::styled(" [i] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)), Span::raw("API Console"),
    ]);

    let hotkeys_block = Paragraph::new(core_hotkeys)
        .block(Block::default().borders(Borders::ALL).style(Style::default().fg(Color::DarkGray)))
        .alignment(ratatui::layout::Alignment::Center);
    f.render_widget(hotkeys_block, main_chunks[3]);

    // 7. Dynamic Config Tuner (Popup Overlay)
    if app.show_tuner {
        let area = centered_rect_absolute(58, 17, f.area()); 
        f.render_widget(Clear, area); 

        let s = |idx| if app.tuner_selected == idx { Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD).add_modifier(Modifier::REVERSED) } else { Style::default().fg(Color::Gray) };
        let on_off = |b| if b { "ON " } else { "OFF" };
        let cache_types = ["f16", "q8_0", "q4_0", "q4_1"];

        let mut text = vec![Line::from("")];

        // DYNAMIC PAGE RENDERING
        match app.tuner_page {
            0 => {
                text.push(Line::from(Span::styled(" --- [ PAGE 1: COMPUTE & MEMORY ] ---", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD))).alignment(ratatui::layout::Alignment::Center));
                text.push(Line::from(Span::styled(format!("  GPU Layers (ngl):       {:<8} [< / >]  ", app.current_ngl), s(0))).alignment(ratatui::layout::Alignment::Center));
                text.push(Line::from(Span::styled(format!("  Context Size (ctx):     {:<8} [< / >]  ", app.current_ctx), s(1))).alignment(ratatui::layout::Alignment::Center));
                text.push(Line::from(Span::styled(format!("  CPU Threads (threads):  {:<8} [< / >]  ", app.current_threads), s(2))).alignment(ratatui::layout::Alignment::Center));
                text.push(Line::from(Span::styled(format!("  Batch Size (n_batch):   {:<8} [< / >]  ", app.current_batch), s(3))).alignment(ratatui::layout::Alignment::Center));
                text.push(Line::from(Span::styled(format!("  Parallel Slots (np):    {:<8} [< / >]  ", app.current_parallel), s(4))).alignment(ratatui::layout::Alignment::Center));
                text.push(Line::from(Span::styled(format!("  Flash Attention:        {:<8} [< / >]  ", on_off(app.flash_attn)), s(5))).alignment(ratatui::layout::Alignment::Center));
                text.push(Line::from(Span::styled(format!("  Memory Lock (mlock):    {:<8} [< / >]  ", on_off(app.mlock)), s(6))).alignment(ratatui::layout::Alignment::Center));
                text.push(Line::from(Span::styled(format!("  No Mem Map (no_mmap):   {:<8} [< / >]  ", on_off(app.no_mmap)), s(7))).alignment(ratatui::layout::Alignment::Center));
                text.push(Line::from(Span::styled(format!("  KV Cache (K-Type):      {:<8} [< / >]  ", cache_types[app.cache_k_idx]), s(8))).alignment(ratatui::layout::Alignment::Center));
                text.push(Line::from(Span::styled(format!("  KV Cache (V-Type):      {:<8} [< / >]  ", cache_types[app.cache_v_idx]), s(9))).alignment(ratatui::layout::Alignment::Center));
            },
            1 => {
                text.push(Line::from(Span::styled(" --- [ PAGE 2: CONTEXT & CACHING ] ---", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD))).alignment(ratatui::layout::Alignment::Center));
                text.push(Line::from(Span::styled(format!("  RoPE Freq Base:         {:<8} [< / >]  ", app.rope_base), s(0))).alignment(ratatui::layout::Alignment::Center));
                text.push(Line::from(Span::styled(format!("  RoPE Scale Factor:      {:<8.2} [< / >]  ", app.rope_scale), s(1))).alignment(ratatui::layout::Alignment::Center));
                text.push(Line::from(Span::styled(format!("  Defrag Threshold:       {:<8.2} [< / >]  ", app.defrag_thold), s(2))).alignment(ratatui::layout::Alignment::Center));
                text.push(Line::from(Span::styled(format!("  Draft Max Tokens:       {:<8} [< / >]  ", app.draft_max), s(3))).alignment(ratatui::layout::Alignment::Center));
                text.push(Line::from(Span::styled(format!("  Draft Min Tokens:       {:<8} [< / >]  ", app.draft_min), s(4))).alignment(ratatui::layout::Alignment::Center));
                text.push(Line::from(Span::styled(format!("  Prompt Cache (Disk):    {:<8} [< / >]  ", on_off(app.prompt_cache)), s(5))).alignment(ratatui::layout::Alignment::Center));
                text.push(Line::from(Span::styled(format!("  Cache All (Chat Hist):  {:<8} [< / >]  ", on_off(app.prompt_cache_all)), s(6))).alignment(ratatui::layout::Alignment::Center));
                text.push(Line::from("")); // Reduced spacers from 5 to 3 to maintain exact box height
                text.push(Line::from(""));
                text.push(Line::from(""));
            },
            2 => {
                text.push(Line::from(Span::styled(" --- [ PAGE 3: DEFAULT SAMPLING ] ---", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD))).alignment(ratatui::layout::Alignment::Center));
                text.push(Line::from(Span::styled(format!("  Temperature:            {:<8.2} [< / >]  ", app.temp), s(0))).alignment(ratatui::layout::Alignment::Center));
                text.push(Line::from(Span::styled(format!("  Top-K:                  {:<8} [< / >]  ", app.top_k), s(1))).alignment(ratatui::layout::Alignment::Center));
                text.push(Line::from(Span::styled(format!("  Top-P:                  {:<8.2} [< / >]  ", app.top_p), s(2))).alignment(ratatui::layout::Alignment::Center));
                text.push(Line::from(Span::styled(format!("  Min-P:                  {:<8.2} [< / >]  ", app.min_p), s(3))).alignment(ratatui::layout::Alignment::Center));
                text.push(Line::from(Span::styled(format!("  Repeat Penalty:         {:<8.2} [< / >]  ", app.rep_pen), s(4))).alignment(ratatui::layout::Alignment::Center));
                text.push(Line::from("")); // Spacers
                text.push(Line::from(""));
                text.push(Line::from(""));
                text.push(Line::from(""));
                text.push(Line::from(""));
            },
            _ => {}
        }

        text.push(Line::from(""));
        text.push(Line::from(Span::styled("[TAB] Next Page   |   [ENTER] Apply   |   [ESC] Cancel", Style::default().fg(Color::DarkGray))).alignment(ratatui::layout::Alignment::Center));

        let title = format!(" Deep router.ini Tuner [Page {}/3] ", app.tuner_page + 1);
        let popup_block = Paragraph::new(text)
            .block(Block::default().title(title).borders(Borders::ALL).style(Style::default().fg(Color::Cyan)));
        
        f.render_widget(popup_block, area);
    }

    // 8. Live Model Selector (Popup Overlay)
    if app.show_model_selector {
        // --- NEW: Dynamic absolute height based on model count (min 5, max 20 lines) ---
        let height = (app.available_models.len() as u16 + 2).clamp(5, 20);
        let area = centered_rect_absolute(60, height, f.area());
        
        f.render_widget(Clear, area);

        let mut items = Vec::new();
        if app.available_models.is_empty() {
            // --- NEW: Center the loading text ---
            items.push(ListItem::new(Line::from("Scanning for models...").alignment(ratatui::layout::Alignment::Center)));
        } else {
            for (i, model) in app.available_models.iter().enumerate() {
                let style = if i == app.model_selector_index {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD).add_modifier(Modifier::REVERSED)
                } else {
                    Style::default().fg(Color::White)
                };
                
                // --- NEW: Center-align the text inside the list, creating a clean "pill" highlight ---
                items.push(ListItem::new(
                    Line::from(Span::styled(format!("  {}  ", model), style))
                        .alignment(ratatui::layout::Alignment::Center)
                ));
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

    // 11. Interactive Help Overlay
    if app.show_help {
        let area = centered_rect_absolute(65, 20, f.area());
        f.render_widget(Clear, area);

        let help_text = vec![
            Line::from(""),
            Line::from(Span::styled(" --- Navigation & Display ---", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
            Line::from("  [q] Quit        | [PgUp/PgDn] Scroll Logs"),
            Line::from("  [g] GPU Metrics | [c] CPU & RAM Metrics"),
            Line::from(""),
            Line::from(Span::styled(" --- Orchestration & Tuning ---", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
            Line::from("  [m] Target Model Selector"),
            Line::from("  [t] Config Tuner (ngl, ctx, threads, batch, parallel)"),
            Line::from("  [i] API Interrogator (Press Esc to exit Insert Mode)"),
            Line::from("  [Up/Dn] Cycle Interrogator Payload History"),
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