use ratatui::{Frame,layout::{Layout,Constraint},style::{Color,Style,Modifier},text::{Line,Span},widgets::{Paragraph,Block,Borders}};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;
use crate::app::{App,State};
use code_racer_protocol::{now_ms,grapheme_count};
pub fn draw(frame:&mut Frame,app:&App) {
    let area=frame.area();
    if area.width<80 || area.height<20 {frame.render_widget(Paragraph::new("Terminal too small.\nMinimum recommended size: 80x20"),area);return;}
    let mono=app.storage.config.theme=="mono";
    let accent=if mono {Color::White} else {Color::Rgb(133,163,184)};
    let muted=if mono {Color::Gray} else {Color::Rgb(101,112,124)};
    let bg=if app.storage.config.theme=="dark" {Color::Rgb(16,18,22)} else {Color::Rgb(27,31,37)};
    frame.render_widget(Block::default().style(Style::default().bg(bg).fg(Color::Gray)),area);
    let sections=Layout::vertical([Constraint::Length(1),Constraint::Length(2),Constraint::Min(10),Constraint::Length(2),Constraint::Length(1)]).split(area);
    frame.render_widget(Paragraph::new(Line::from(vec![Span::styled(" code-racer",Style::default().fg(accent).add_modifier(Modifier::BOLD)),Span::raw(format!("                                              {:?}",app.state))])),sections[0]);
    let filename=match app.language.as_str() {"rust"=>"main.rs","python"=>"app.py","typescript"|"javascript"=>"index.ts","sql"=>"query.sql",_=>"src/session.rs"};
    frame.render_widget(Paragraph::new(format!(" {filename}" )).style(Style::default().fg(muted)),sections[1]);
    let mut lines:Vec<Line>=Vec::new();
    match app.state {
        State::Home=>{lines.push(Line::raw("  Practice with intent. Race with your team."));lines.push(Line::raw(""));menu(&mut lines,&["Solo","Multiplayer","History","Settings","Help","Quit"],app.selected,accent);}
        State::SoloConfig=>{let items=[format!("Mode       {}",app.mode),format!("Language   {}",app.language),format!("Words      {}",app.words),format!("Seconds    {}",app.seconds),"Start session".into()];for (i,item) in items.iter().enumerate() {lines.push(item_line(item,i==app.selected,accent));}lines.push(Line::raw("\n  h/l change value · Enter starts"));}
        State::Username=>lines.extend([Line::raw("  First launch: choose your username (1–24 characters)"),Line::raw(format!("  > {}_",app.input)),Line::raw("  Enter saves · Esc exits")]),
        State::JoinRoom=>lines.extend([Line::raw("  Room code:"),Line::raw(format!("  > {}_",app.input)),Line::raw("  Enter connects · Esc returns")]),
        State::CreateRoom=>lines.extend([Line::raw("  Create a private room"),Line::raw(format!("  Server: {}",app.storage.config.multiplayer.server)),Line::raw("  Enter connects · Esc returns")]),
        State::MultiplayerMenu=>{menu(&mut lines,&["Create room [c]","Join room [j]"],app.selected,accent);lines.push(Line::raw(format!("\n  Server: {}",app.storage.config.multiplayer.server)));}
        State::Lobby|State::Countdown=>{if let Some(room)=&app.room {lines.push(Line::raw(format!("  Room: {}  ·  {}",room.code,room.language)));lines.push(Line::raw(""));for p in &room.players {lines.push(Line::raw(format!("  {:24} {:9} {}",p.name,if p.ready {"READY"} else {"WAITING"},if p.id==room.host {"host"} else {""})));}lines.push(Line::raw(""));if app.state==State::Countdown {let remaining=room.start_ms.unwrap_or(0) as i64-now_ms() as i64-app.offset;lines.push(Line::raw(format!("  Starting in {}…",(remaining.max(0)+999)/1000)));} else {lines.push(Line::raw("  [R] Toggle ready · [S] Host starts · Esc leaves"));}}}
        State::SoloRace|State::MultiplayerRace=>{
            if let Some(s)=&app.session {
                let typed:Vec<_>=s.typed.graphemes(true).collect();let width=sections[2].width.saturating_sub(8) as usize;
                let mut rows:Vec<Vec<Span>>=vec![Vec::new()];let mut col=0;let mut cursor_row=0;
                for (i,g) in s.target.iter().enumerate() {
                    if i==typed.len() {cursor_row=rows.len()-1;}
                    let style=if i<typed.len() {if typed[i]==g {Style::default().fg(if mono {Color::White} else {Color::Rgb(163,190,140)})} else {Style::default().fg(if mono {Color::White} else {Color::Rgb(191,113,119)}).add_modifier(Modifier::UNDERLINED)}} else if i==typed.len() {Style::default().bg(accent).fg(bg)} else {Style::default().fg(muted)};
                    if g=="\n" {rows.last_mut().expect("row").push(Span::styled("↵",style));rows.push(Vec::new());col=0;continue;}
                    let w=g.width();if col+w>width {rows.push(Vec::new());col=0;if i==typed.len() {cursor_row=rows.len()-1;}}
                    rows.last_mut().expect("row").push(Span::styled(g.clone(),style));col+=w;
                }
                let reserve=if app.room.is_some() {app.room.as_ref().map_or(0,|r|r.players.len()+2)} else {0};
                let height=(sections[2].height as usize).saturating_sub(reserve).max(1);let start=cursor_row.saturating_sub(height/2);
                for (i,row) in rows.into_iter().enumerate().skip(start).take(height) {let mut spans=vec![Span::styled(format!(" {:3} │ ",i+1),Style::default().fg(muted))];spans.extend(row);lines.push(Line::from(spans));}
                if let Some(room)=&app.room {lines.push(Line::raw(""));for p in room.ranking() {let percent=p.position as f64/grapheme_count(&room.text).max(1) as f64;let filled=(percent*20.0).round() as usize;lines.push(Line::raw(format!(" {:24} {}{} {:3.0}% {:6.1} WPM {}",p.name,"━".repeat(filled.min(20)),"─".repeat(20-filled.min(20)),percent*100.0,p.wpm,if !p.connected {"offline"} else if p.finished_ms.is_some() {"done"} else {""})));}}
            }
        }
        State::Results=>{
            if let Some(room)=&app.room {lines.push(Line::raw("  POSITION   PLAYER                     WPM       ACC      TIME"));for (i,p) in room.ranking().iter().enumerate() {lines.push(Line::raw(format!("  {:8}   {:24} {:7.1}    {:5.1}%   {}",i+1,p.name,p.wpm,p.accuracy,p.finished_ms.map(|ms|format!("{:.2}s",ms as f64/1000.0)).unwrap_or("DNF".into())));}lines.push(Line::raw("\n  [R/L] Host returns to lobby · Esc leaves · Q quits"));}
            else if let Some(s)=&app.session {let st=s.stats();lines.extend([Line::raw(format!("  Session complete · {} / {}",app.mode,app.language)),Line::raw(format!("\n  WPM         {:.1}\n  Raw WPM     {:.1}\n  Accuracy    {:.2}%\n  Errors      {}\n  Duration    {:.2}s",st.wpm,st.raw_wpm,st.accuracy,st.errors,st.elapsed)),Line::raw("\n  [R] Restart · Esc home · Q quit")]);}
        }
        State::Settings=>{lines.push(item_line(&format!("Username   {}{}",if app.selected==0 {&app.input} else {&app.storage.config.username},if app.selected==0 {"_"} else {""}),app.selected==0,accent));lines.push(item_line(&format!("Theme      {}",app.storage.config.theme),app.selected==1,accent));lines.push(item_line(&format!("Language   {}",app.language),app.selected==2,accent));lines.push(Line::raw("\n  Enter saves username · j/k navigate · l changes value"));}
        State::Help=>lines.extend([Line::raw("  j/k navigate · Enter select · Esc return · q quit · ? help"),Line::raw("  Solo: h/l change mode, language, count, duration"),Line::raw("  Typing: printable keys write · Backspace corrects"),Line::raw("  Code: Enter newline · Tab inserts four spaces"),Line::raw("  Lobby: r ready · s host starts"),Line::raw("  Results: r restart / host returns to lobby"),Line::raw("  Ctrl+C exits and restores your terminal"),Line::raw("  Server address can be changed with --server or config.toml")]),
        State::History=>{
            let history=&app.storage.history;let best=history.iter().map(|r|r.stats.wpm).fold(0.0,f64::max);let accuracy=history.iter().map(|r|r.stats.accuracy).fold(0.0,f64::max);let total:f64=history.iter().map(|r|r.stats.elapsed).sum();let avg=history.iter().map(|r|r.stats.wpm).sum::<f64>()/history.len().max(1) as f64;
            lines.push(Line::raw(format!("  {} sessions · best {:.1} · average {:.1} WPM · best accuracy {:.1}% · {:.0}s",history.len(),best,avg,accuracy,total)));lines.push(Line::raw("  DATE        MODE           LANG         WPM      ACC     ERR"));for r in history.iter().rev().skip(app.selected).take(sections[2].height.saturating_sub(3) as usize) {lines.push(Line::raw(format!("  {:.10}  {:14} {:10} {:7.1}  {:6.1}%  {}",r.date,r.mode,r.language,r.stats.wpm,r.stats.accuracy,r.stats.errors)));}
        }
    }
    frame.render_widget(Paragraph::new(lines),sections[2]);
    frame.render_widget(Paragraph::new(if app.error.is_empty() {String::new()} else {format!(" E: {}",app.error)}).style(Style::default().fg(if mono {Color::White} else {Color::Rgb(191,113,119)})).block(Block::default().borders(Borders::TOP)),sections[3]);
    let status=if let Some(s)=&app.session {let st=s.stats();format!(" INSERT │ {} │ {:.1} WPM │ raw {:.1} │ {:.1}% │ errors: {} │ {:02}:{:02} │ {:.0}%",app.language,st.wpm,st.raw_wpm,st.accuracy,st.errors,st.elapsed as u64/60,st.elapsed as u64%60,st.position as f64/st.length.max(1) as f64*100.0)} else {format!(" NORMAL │ {} │ {} │ j/k navigate · Enter select · ? help",app.storage.config.username,app.language)};
    frame.render_widget(Paragraph::new(status).style(Style::default().bg(accent).fg(bg)),sections[4]);
}
fn item_line(item:&str,selected:bool,accent:Color)->Line<'static> {Line::styled(format!(" {} {}",if selected {">"} else {" "},item),Style::default().fg(if selected {accent} else {Color::Gray}))}
fn menu(lines:&mut Vec<Line<'static>>,items:&[&str],selected:usize,accent:Color) {for (i,item) in items.iter().enumerate() {lines.push(item_line(item,i==selected,accent));}}
