use crate::helper::scrollbar::{self, SCROLLBAR_W, ScrollbarIds};
use crate::helper::window_chrome::fill;
use crate::helper::window_chrome::{
    FOOTER_TEX, SYS_BASE_OFF_TEX, SYS_BASE_ON_TEX, TITLEBAR_TEX, draw_footer, draw_sys_button,
    draw_titlebar, text_color,
};
use crate::{BuildCtx, InGameWindow, Window};
use nostalre_game::data_table::DataTable;
use nostalre_game::event::GameEvent;
use nostalre_game::quest::Quest;
use nostalre_game::quest::QuestLog;
use nostalre_ui::draw::{self, DrawCall, TexturePath, TextureRef, strip_color_codes, word_wrap};
use nostalre_game::quest_guidance;
use nostalre_ui::frame::{ButtonTextures, UiFrame, WidgetId};
use nostalre_ui::rect::Rect;

/// Default top, under the basic info window and its menu
const DEFAULT_Y: f32 = 240.0;
pub const QUEST_WINDOW_ID: WidgetId = WidgetId(3700);
const CLOSE_BTN_ID: WidgetId = WidgetId(3701);
const VIEW_BTN_ID: WidgetId = WidgetId(3702);
const FOOTER_CLOSE_BTN_ID: WidgetId = WidgetId(3703);
const TAB_BASE_ID: u32 = 3710;
const ROW_BASE_ID: u32 = 3720;
const LIST_SCROLL: ScrollbarIds = ScrollbarIds {
    up: WidgetId(3704),
    down: WidgetId(3705),
    thumb: WidgetId(3706),
};

pub const QUEST_DETAIL_WINDOW_ID: WidgetId = WidgetId(3750);
const DETAIL_CLOSE_BTN_ID: WidgetId = WidgetId(3751);
const DETAIL_SCROLL: ScrollbarIds = ScrollbarIds {
    up: WidgetId(3752), down: WidgetId(3753), thumb: WidgetId(3754),
};

const CLOSE_OFF_TEX: &str = nostalre_resources::ui::basic::SYS_CLOSE_OFF;
const CLOSE_ON_TEX: &str = nostalre_resources::ui::basic::SYS_CLOSE_ON;
const TAB_QUE_TEX: [&str; 3] = [
    nostalre_resources::ui::basic::TAB_QUE_01,
    nostalre_resources::ui::basic::TAB_QUE_02,
    nostalre_resources::ui::basic::TAB_QUE_03,
];
const DETAIL_BG_TEX: &str = nostalre_resources::ui::basic::QUEST_WINDOW;

const VIEW_BTN: ButtonTextures = ButtonTextures {
    normal: nostalre_resources::ui::BTN_VIEW,
    hover: nostalre_resources::ui::BTN_VIEW_A,
    pressed: nostalre_resources::ui::BTN_VIEW_B,
};
const CLOSE_BTN: ButtonTextures = ButtonTextures {
    normal: nostalre_resources::ui::BTN_CLOSE,
    hover: nostalre_resources::ui::BTN_CLOSE_A,
    pressed: nostalre_resources::ui::BTN_CLOSE_B,
};

const WIN_W: f32 = 280.0;
const WIN_H: f32 = 200.0;
const TITLE_H: f32 = 17.0;
const FOOTER_H: f32 = 24.0;
const CLOSE_BTN_SIZE: f32 = 11.0;
const TAB_STRIP_W: f32 = 52.0;
const TAB_STRIP_H: f32 = 94.0;
const TAB_COUNT: usize = 3;
const ROW_H: f32 = 36.0;
const ICON: f32 = 24.0;

const DETAIL_W: f32 = 350.0;
const DETAIL_H: f32 = 375.0;
const DETAIL_IMG_W: f32 = 90.0;
const DETAIL_IMG_H: f32 = 60.0;
const DESC_W: f32 = DETAIL_W - 44.0;

const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const PANEL_BG: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const BORDER: [f32; 4] = [0.761, 0.761, 0.761, 1.0];
const SELECTION_COLOR: [f32; 4] = [0.451, 0.62, 0.937, 1.0];
const TAB_BAR_BG: [f32; 4] = [0.71, 0.714, 0.71, 1.0];
const TAB_INACTIVE: [f32; 4] = [0.808, 0.808, 0.808, 1.0];
const TEXT: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
const LABEL_COLOR: [f32; 4] = [0.14, 0.20, 0.38, 1.0];

const TAB_LABELS: [&str; TAB_COUNT] = [crate::pt!("On", "Ativo"), crate::pt!("Off", "Oculto"), crate::pt!("All", "Todos")];

#[derive(Clone, Copy, PartialEq, Eq)]
enum QuestTab {
    On = 0,
    Off = 1,
    All = 2,
}

impl QuestTab {
    fn from_index(i: usize) -> Self {
        match i {
            0 => Self::On,
            1 => Self::Off,
            _ => Self::All,
        }
    }

    fn matches(self, quest: &Quest) -> bool {
        match self {
            Self::On => quest.active,
            Self::Off => !quest.active,
            Self::All => true,
        }
    }
}

fn textured(ui: &mut UiFrame, x: f32, y: f32, w: f32, h: f32, path: impl Into<TexturePath>) {
    let (v, i) = draw::quad_vertices(x, y, w, h, WHITE);
    ui.draw_calls.push(DrawCall {
        vertices: v.into(),
        indices: i.into(),
        texture: TextureRef::Named(path.into()),
    });
}

fn quest_title(data: &DataTable, id: u32) -> String {
    data.quest_display
        .as_ref()
        .map(|t| t.title(id))
        .unwrap_or_else(|| format!("Quest {id}"))
}

pub struct QuestWindow {
    open: bool,
    has_grf_textures: bool,
    tab: QuestTab,
    scroll: usize,
    selected: Option<u32>,
    pending_toggle: Option<(u32, bool)>,
}

impl Default for QuestWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl QuestWindow {
    pub fn new() -> Self {
        Self {
            open: false,
            has_grf_textures: false,
            tab: QuestTab::On,
            scroll: 0,
            selected: None,
            pending_toggle: None,
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn open(&mut self) {self.open=true;}

    pub fn toggle(&mut self) {
        self.open = !self.open;
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    fn tab_strip(&mut self, ui: &mut UiFrame, x: f32, y: f32, grf: bool) {
        // The original narrow tab strip has untranslated text baked into it.
        let _=grf;
        fill(ui, x, y, TAB_STRIP_W, TAB_STRIP_H, TAB_BAR_BG);
        let tab_h = TAB_STRIP_H / TAB_COUNT as f32;
        for i in 0..TAB_COUNT {
            let ty = y + i as f32 * tab_h;
            let rect = Rect::new(x, ty, TAB_STRIP_W, tab_h);
            let selected = self.tab as usize == i;
            {
                fill(
                    ui,
                    x + 1.0,
                    ty + 1.0,
                    TAB_STRIP_W - 2.0,
                    tab_h - 2.0,
                    if selected { WHITE } else { TAB_INACTIVE },
                );
                ui.text(x + 2.0, ty + tab_h / 2.0 + 4.0, TAB_LABELS[i], TEXT);
            }
            let resp = ui.interact(WidgetId(TAB_BASE_ID + i as u32), rect);
            if resp.hovered() {
                ui.any_interactive_hovered = true;
            }
            if resp.clicked() && !selected {
                self.tab = QuestTab::from_index(i);
                self.scroll = 0;
            }
        }
    }

    fn list_panel(
        &mut self,
        ui: &mut UiFrame,
        quest_log: &QuestLog,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        data: &DataTable,
    ) {
        fill(ui, x, y, w, h, PANEL_BG);

        let ids: Vec<u32> = quest_log
            .quests
            .iter()
            .filter(|q| self.tab.matches(q))
            .map(|q| q.id)
            .collect();

        let visible = (h / ROW_H).max(1.0) as usize;
        let max_scroll = ids.len().saturating_sub(visible);
        let has_bar = max_scroll > 0;
        let bar_w = if has_bar { SCROLLBAR_W } else { 0.0 };
        if has_bar {
            let content_rect = Rect::new(x, y, w, h);
            self.scroll = scrollbar::scrollbar(
                ui,
                LIST_SCROLL,
                self.scroll,
                visible,
                max_scroll,
                content_rect,
                x + w - SCROLLBAR_W,
                y,
                h,
            );
        } else {
            self.scroll = 0;
        }

        for row in 0..visible {
            let Some(&id) = ids.get(self.scroll + row) else {
                break;
            };
            let row_y = y + row as f32 * ROW_H;
            let row_rect = Rect::new(x, row_y, w - bar_w, ROW_H);
            if self.selected == Some(id) {
                fill(
                    ui,
                    row_rect.x,
                    row_rect.y,
                    row_rect.w,
                    row_rect.h,
                    SELECTION_COLOR,
                );
            }
            fill(ui, x, row_y + ROW_H - 1.0, w - bar_w, 1.0, BORDER);

            let icon_y = row_y + (ROW_H - ICON) / 2.0;
            let icon = data
                .quest_display
                .as_ref()
                .map(|t| t.icon_texture(id))
                .unwrap_or_default();
            if self.has_grf_textures && !icon.is_empty() {
                textured(ui, x + 4.0, icon_y, ICON, ICON, icon);
            } else {
                fill(ui, x + 4.0, icon_y, ICON, ICON, TAB_INACTIVE);
            }

            let title = quest_title(data, id);
            let title_x=x+4.0+ICON+6.0;
            for (index,line) in wrap_quest_text(ui,&title,w-bar_w-(title_x-x)-4.0).iter().take(2).enumerate() {
                ui.text(title_x,row_y+14.0+index as f32*14.0,line,TEXT);
            }

            let resp = ui.interact(WidgetId(ROW_BASE_ID + row as u32), row_rect);
            if resp.hovered() {
                ui.any_interactive_hovered = true;
            }
            if resp.clicked() {
                self.selected = Some(id);
            }
            if resp.right_clicked() {
                self.selected = Some(id);
                let active = quest_log
                    .quests
                    .iter()
                    .find(|q| q.id == id)
                    .map(|q| q.active)
                    .unwrap_or(false);
                self.pending_toggle = Some((id, !active));
            }
        }
    }
}

impl Window for QuestWindow {
    fn has_grf_textures(&self) -> bool {
        self.has_grf_textures
    }
    fn set_has_grf_textures(&mut self, value: bool) {
        self.has_grf_textures = value;
    }
    fn window_size(&self) -> (f32, f32) {
        (WIN_W, WIN_H)
    }
    fn grf_texture_paths() -> Vec<&'static str> {
        let mut paths = vec![
            TITLEBAR_TEX,
            FOOTER_TEX,
            SYS_BASE_OFF_TEX,
            SYS_BASE_ON_TEX,
            CLOSE_OFF_TEX,
            CLOSE_ON_TEX,
            TAB_QUE_TEX[0],
            TAB_QUE_TEX[1],
            TAB_QUE_TEX[2],
            VIEW_BTN.normal,
            VIEW_BTN.hover,
            VIEW_BTN.pressed,
            CLOSE_BTN.normal,
            CLOSE_BTN.hover,
            CLOSE_BTN.pressed,
        ];
        paths.extend(scrollbar::grf_texture_paths());
        paths
    }
}

impl InGameWindow for QuestWindow {
    fn owns_keyboard(&self, _ctx: &BuildCtx) -> bool {
        self.open
    }

    fn wants_escape(&self, _ctx: &BuildCtx) -> bool {
        self.open
    }

    fn on_escape(&mut self, _ctx: &mut BuildCtx) -> Vec<GameEvent> {
        self.open = false;
        Vec::new()
    }

    fn build(&mut self, ui: &mut UiFrame, ctx: &mut BuildCtx) -> Vec<GameEvent> {
        let data = ctx.data;
        let quest_log = ctx.quest_log;
        if !self.open {
            return Vec::new();
        }
        if let Some(sel) = self.selected
            && !quest_log.quests.iter().any(|q| q.id == sel)
        {
            self.selected = None;
        }
        let mut events = Vec::new();
        let prev_grf = ui.has_grf_textures;
        ui.has_grf_textures = self.has_grf_textures;
        let grf = self.has_grf_textures;
        let tc = text_color(grf);
        self.pending_toggle = None;

        let win = ui.window_at(QUEST_WINDOW_ID, WIN_W, WIN_H, TITLE_H, 0.0, DEFAULT_Y);
        let (x, y) = (win.x, win.y);
        ui.interact(QUEST_WINDOW_ID, Rect::new(x, y, WIN_W, WIN_H));

        draw_titlebar(ui, x, y, WIN_W, TITLE_H, grf);
        ui.text(x + 16.0, y + TITLE_H - 3.0, crate::pt!("Quest List", "Lista de missões"), tc);

        let close_rect = Rect::new(
            x + WIN_W - CLOSE_BTN_SIZE - 3.0,
            y + (TITLE_H - CLOSE_BTN_SIZE) / 2.0,
            CLOSE_BTN_SIZE,
            CLOSE_BTN_SIZE,
        );
        let close_resp = ui.interact(CLOSE_BTN_ID, close_rect);
        if close_resp.hovered() {
            ui.any_interactive_hovered = true;
        }
        draw_sys_button(
            ui,
            close_rect,
            (CLOSE_BTN_SIZE, CLOSE_BTN_SIZE),
            close_resp.hovered(),
            grf,
            CLOSE_ON_TEX,
            CLOSE_OFF_TEX,
            Some('x'),
        );
        if close_resp.clicked() {
            self.open = false;
            ui.has_grf_textures = prev_grf;
            return events;
        }

        let body_y = y + TITLE_H;
        let body_h = WIN_H - TITLE_H - FOOTER_H;
        self.tab_strip(ui, x, body_y, grf);
        let ids: Vec<_> = quest_log.quests.iter().filter(|q| self.tab.matches(q)).map(|q|q.id).collect();
        if self.selected.is_none_or(|id| !ids.contains(&id)) {
            self.selected = quest_log.followed().map(|q|q.id).filter(|id|ids.contains(id)).or_else(||ids.first().copied());
        }
        if ui.ctx.key_up || ui.ctx.key_down {
            if let Some(index) = self.selected.and_then(|id|ids.iter().position(|q|*q==id)) {
                let index = if ui.ctx.key_up {index.saturating_sub(1)} else {(index+1).min(ids.len()-1)};
                self.selected = Some(ids[index]);
                let visible = (body_h / ROW_H).max(1.0) as usize;
                if index < self.scroll {self.scroll=index;}
                if index >= self.scroll+visible {self.scroll=index+1-visible;}
            }
        }
        let list_x = x + TAB_STRIP_W;
        let list_w = WIN_W - TAB_STRIP_W;
        self.list_panel(ui, quest_log, list_x, body_y, list_w, body_h, data);
        fill(ui, x + TAB_STRIP_W, body_y, 1.0, body_h, BORDER);

        let footer_y = y + WIN_H - FOOTER_H;
        draw_footer(ui, x, footer_y, WIN_W, FOOTER_H, grf);
        let view_rect = Rect::new(x + 6.0, footer_y + 2.0, 42.0, 20.0);
        if ui
            .button(VIEW_BTN_ID, view_rect, &VIEW_BTN, crate::pt!("View","Ver"))
            .clicked()
            || ui.ctx.key_enter
        {
            if let Some(id) = self.selected {
                events.push(GameEvent::OpenQuestDetail { quest_id: id });
                if ui.ctx.pad_labels { self.open = false; }
            }
        }
        let campaign=quest_log.quests.iter().find(|q|q.active && (900000..900100).contains(&q.id));
        if let Some(main)=campaign {
            let rect=Rect::new(x+54.0,footer_y+2.0,166.0,20.0);
            ui.text(rect.x+2.0,rect.y+14.0,"Voltar à campanha",tc);
            if ui.interact(WidgetId(3790),rect).clicked() {events.push(GameEvent::OpenQuestDetail {quest_id:main.id});}
        }
        let close2_rect = Rect::new(x + WIN_W - 48.0, footer_y + 2.0, 42.0, 20.0);
        if ui
            .button(FOOTER_CLOSE_BTN_ID, close2_rect, &CLOSE_BTN, crate::pt!("Close", "Fechar"))
            .clicked()
        {
            self.open = false;
            ui.has_grf_textures = prev_grf;
            return events;
        }

        if let Some((quest_id, active)) = self.pending_toggle.take() {
            events.push(GameEvent::RequestToggleQuestActive { quest_id, active });
        }

        ui.has_grf_textures = prev_grf;
        events
    }
}

pub struct QuestDetailWindow {
    open: bool,
    has_grf_textures: bool,
    quest_id: Option<u32>,
    wrapped_desc: Vec<String>,
    desc_source: String,
    scroll: usize,
}

impl Default for QuestDetailWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl QuestDetailWindow {
    pub fn new() -> Self {
        Self {
            open: false,
            has_grf_textures: false,
            quest_id: None,
            wrapped_desc: Vec::new(),
            desc_source: String::new(),
            scroll: 0,
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn quest_id(&self) -> Option<u32> {
        self.quest_id
    }

    pub fn open(&mut self, quest_id: u32) {
        self.quest_id = Some(quest_id);
        self.open = true;
        self.wrapped_desc.clear();
        self.desc_source.clear();
        self.scroll = 0;
    }

    pub fn close(&mut self) {
        self.open = false;
    }
}

impl Window for QuestDetailWindow {
    fn has_grf_textures(&self) -> bool {
        self.has_grf_textures
    }
    fn set_has_grf_textures(&mut self, value: bool) {
        self.has_grf_textures = value;
    }
    fn window_size(&self) -> (f32, f32) {
        (DETAIL_W, DETAIL_H)
    }
    fn grf_texture_paths() -> Vec<&'static str> {
        let mut paths=vec![DETAIL_BG_TEX, CLOSE_OFF_TEX, CLOSE_ON_TEX, TITLEBAR_TEX];
        paths.extend(scrollbar::grf_texture_paths());
        paths
    }
}

impl InGameWindow for QuestDetailWindow {
    fn owns_keyboard(&self, _ctx: &BuildCtx) -> bool {
        self.open
    }

    fn wants_escape(&self, _ctx: &BuildCtx) -> bool {
        self.is_open()
    }

    fn on_escape(&mut self, _ctx: &mut BuildCtx) -> Vec<GameEvent> {
        self.close();
        Vec::new()
    }

    fn build(&mut self, ui: &mut UiFrame, ctx: &mut BuildCtx) -> Vec<GameEvent> {
        let data = ctx.data;
        if !self.open {
            return Vec::new();
        }
        let Some(quest) = self.quest_id.and_then(|id| ctx.quest_log.get(id)) else {
            self.open = false;
            return Vec::new();
        };
        let events = Vec::new();
        let prev_grf = ui.has_grf_textures;
        ui.has_grf_textures = self.has_grf_textures;
        let grf = self.has_grf_textures;

        let win = ui.window_at(
            QUEST_DETAIL_WINDOW_ID,
            DETAIL_W,
            DETAIL_H,
            20.0,
            370.0,
            60.0,
        );
        let (x, y) = (win.x, win.y);
        ui.interact(QUEST_DETAIL_WINDOW_ID, Rect::new(x, y, DETAIL_W, DETAIL_H));

        if grf {
            textured(ui, x, y, DETAIL_W, DETAIL_H, DETAIL_BG_TEX);
        } else {
            fill(ui, x, y, DETAIL_W, DETAIL_H, PANEL_BG);
            fill(ui, x, y, DETAIL_W, 1.0, BORDER);
        }

        // Cover labels baked in Korean/English into the old quest artwork.
        draw_titlebar(ui,x,y,DETAIL_W,20.0,grf);
        ui.text(x+12.0,y+15.0,"Detalhes da missão",text_color(grf));

        let close_rect = Rect::new(x + DETAIL_W - 14.0, y + 4.0, CLOSE_BTN_SIZE, CLOSE_BTN_SIZE);
        let close_resp = ui.interact(DETAIL_CLOSE_BTN_ID, close_rect);
        if close_resp.hovered() {
            ui.any_interactive_hovered = true;
        }
        draw_sys_button(
            ui,
            close_rect,
            (CLOSE_BTN_SIZE, CLOSE_BTN_SIZE),
            close_resp.hovered(),
            grf,
            CLOSE_ON_TEX,
            CLOSE_OFF_TEX,
            Some('x'),
        );
        if close_resp.clicked() {
            self.open = false;
            ui.has_grf_textures = prev_grf;
            return events;
        }

        let id = quest.id;
        if grf {
            let img = data
                .quest_display
                .as_ref()
                .map(|t| t.image_texture(id))
                .unwrap_or_default();
            textured(ui, x + 15.0, y + 30.0, DETAIL_IMG_W, DETAIL_IMG_H, img);
        } else {
            fill(
                ui,
                x + 15.0,
                y + 30.0,
                DETAIL_IMG_W,
                DETAIL_IMG_H,
                TAB_INACTIVE,
            );
        }

        let paper=[0.96,0.94,0.87,1.0];
        let line_h=ui.atlas.line_height.max(14.0)+2.0;
        fill(ui,x+110.0,y+25.0,DETAIL_W-120.0,70.0,paper);
        for (index,line) in wrap_quest_text(ui,&quest_title(data,id),DETAIL_W-136.0).iter().take(3).enumerate() {
            ui.text(x+116.0,y+38.0+index as f32*line_h,line,LABEL_COLOR);
        }
        let guide=quest_guidance::for_quest(ctx.quest_log,id);
        let table=data.quest_display.as_ref();
        let summary=table.map(|t|t.summary(id)).unwrap_or_default();
        let description=table.map(|t|t.description(id)).unwrap_or_default();
        let mut source=String::new();
        // The actionable destination comes first; visiting receipts stay useful
        // even when the journal's parent visibility toggle has been disabled.
        if let Some(guide)=guide {
            source.push_str(&format!("PRÓXIMO PASSO\n{}\n{}: {} ({}, {})\n\n",guide.objective,guide.destination.name,guide.destination.map,guide.destination.x,guide.destination.y));

        }
        if !summary.trim().is_empty() {source.push_str(&format!("RESUMO\n{}\n\n",plain_quest_text(&summary)));}
        if !description.trim().is_empty() {source.push_str(&format!("DETALHES\n{}\n\n",plain_quest_text(&description)));}
        if !quest.objectives.is_empty() {
            source.push_str("OBJETIVOS\n");
            for objective in &quest.objectives {
                source.push_str(&format!("{}: {} / {}\n",objective.name,objective.current,objective.required));
            }
        }
        if let Some(end)=quest.end_time {source.push_str(&format!("\nPrazo: {}",format_end_time(end)));}
        if source.trim().is_empty() {source="Sem descrição disponível para esta missão.".into();}
        if self.desc_source!=source {
            self.wrapped_desc=wrap_quest_text(ui,&source,DESC_W);
            self.desc_source=source;
        }
        let content=Rect::new(x+14.0,y+106.0,DETAIL_W-28.0,DETAIL_H-132.0);
        fill(ui,content.x-3.0,content.y-5.0,content.w+6.0,content.h+10.0,paper);
        let visible=(content.h/line_h).floor() as usize;
        let max_scroll=self.wrapped_desc.len().saturating_sub(visible);
        self.scroll=self.scroll.min(max_scroll);
        if ui.ctx.key_up {self.scroll=self.scroll.saturating_sub(1);}
        if ui.ctx.key_down {self.scroll=(self.scroll+1).min(max_scroll);}
        self.scroll=scrollbar::scrollbar(ui,DETAIL_SCROLL,self.scroll,visible,max_scroll,content,
            content.x+content.w-SCROLLBAR_W,content.y,content.h);
        for (row,line) in self.wrapped_desc.iter().skip(self.scroll).take(visible).enumerate() {
            let color=if matches!(line.as_str(),"PRÓXIMO PASSO"|"CONTINUAÇÃO DO TUTORIAL"|"RESUMO"|"DETALHES"|"OBJETIVOS") {LABEL_COLOR} else {TEXT};
            ui.text(content.x,content.y+ui.atlas.ascent+row as f32*line_h,line,color);
        }
        fill(ui,x+9.0,y+DETAIL_H-20.0,DETAIL_W-18.0,16.0,paper);
        ui.text(x+14.0,y+DETAIL_H-7.0,"↑↓ Rolar    B Voltar",LABEL_COLOR);

        ui.has_grf_textures = prev_grf;
        events
    }
}

fn plain_quest_text(text:&str)->String {
    use nostalre_game::npc_text::{self,NpcTextPart};
    let text=npc_text::parse(text).into_iter().filter_map(|part|match part {
        NpcTextPart::Text(t)=>Some(t),NpcTextPart::Link{label,..}=>Some(label),_=>None,
    }).collect::<String>();
    strip_color_codes(&text).trim().to_string()
}

fn wrap_quest_text(ui:&UiFrame,text:&str,width:f32)->Vec<String> {
    word_wrap(&plain_quest_text(text),width,|s|ui.atlas.measure_text(s),false).into_iter()
        .flat_map(|line|if ui.atlas.measure_text(&line)>width {
            word_wrap(&line,width,|s|ui.atlas.measure_text(s),true)
        }else{vec![line]}).collect()
}

fn format_end_time(end: u32) -> String {
    let secs = end as i64;
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let hour = rem / 3600;
    let minute = (rem % 3600) / 60;

    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { year + 1 } else { year };

    format!("~ {year:04}/{month:02}/{day:02} {hour:02}:{minute:02}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostalre_game::character::Character;
    use nostalre_game::data_table::DataTable;
    use nostalre_game::quest::{Quest, QuestObjective};

    use nostalre_ui::context::UiContext;
    use nostalre_ui::state::StateCache;
    use nostalre_ui::test_support::test_frame;

    #[test]
    fn quest_detail_keeps_long_summary_and_description_inside_the_window() {
        use nostalre_formats::lua_table::QuestDisplay;
        use nostalre_game::data_table::quest_display_table::QuestDisplayTable;
        let mut data=DataTable::new();
        data.quest_display=Some(QuestDisplayTable::from_entries([(900_001,QuestDisplay {
            title:"Uma missão com um título suficientemente longo para precisar de duas linhas".into(),
            summary:"You arrived at the bridge to Prontera but didn't find any hidden presents. Find the other places and return to the Information Staff after visiting all three locations.".into(),
            description:"Descrição longa com acentos: ação, missão, Poção Vermelha e orientação. ".repeat(30),
            ..Default::default()
        })].into()));
        let mut log=QuestLog::default();log.add(Quest{id:900_001,active:true,..Default::default()});
        let mut character=Character::new();let mut state=StateCache::new();
        let mut input=UiContext::new(854.0,480.0);input.pad_labels=true;
        let mut ui=test_frame(&mut input,&mut state);
        let mut ctx=crate::BuildCtx::test(&mut character,&data);ctx.quest_log=&log;
        let mut window=QuestDetailWindow::new();window.open(900_001);
        window.build(&mut ui,&mut ctx);
        let bounds=ui.state.get::<nostalre_ui::frame::WindowState>(QUEST_DETAIL_WINDOW_ID).unwrap();
        for call in &ui.draw_calls {
            if !matches!(call.texture,TextureRef::FontAtlas) {continue;}
            for vertex in call.vertices.iter() {
                assert!(vertex.position[0]>=bounds.x && vertex.position[0]<=bounds.x+DETAIL_W,
                    "quest text escapes horizontally: {:?}, window x={} width={}",vertex.position,bounds.x,DETAIL_W);
                assert!(vertex.position[1]>=bounds.y && vertex.position[1]<=bounds.y+DETAIL_H,
                    "quest text escapes vertically: {:?}, window y={} height={}",vertex.position,bounds.y,DETAIL_H);
            }
        }
    }

    #[test]
    fn classic_hidden_quest_can_be_read_and_scrolled_without_changing_its_state() {
        let mut data=DataTable::new();
        data.quest_display=Some(nostalre_game::data_table::quest_display_table::QuestDisplayTable::from_entries(
            [(900_002,nostalre_formats::lua_table::QuestDisplay {
                title:"Treinamento classico".into(),
                description:"Leia as instrucoes do treinamento e fale com o instrutor. ".repeat(30),
                ..Default::default()
            })].into()));
        let mut character=Character::new();let mut state=StateCache::new();
        let mut log=QuestLog::default();
        log.add(Quest{id:900_002,active:false,..Default::default()});
        let mut window=QuestDetailWindow::new();window.open(900_002);
        for _ in 0..100 {
            let mut input=UiContext::new(854.0,480.0);input.pad_labels=true;input.key_down=true;
            let mut ui=test_frame(&mut input,&mut state);
            let mut ctx=crate::BuildCtx::test(&mut character,&data);ctx.quest_log=&log;
            window.build(&mut ui,&mut ctx);
        }
        assert!(window.desc_source.contains("instrucoes do treinamento"));
        assert!(!window.desc_source.contains("Academia"));
        assert!(window.scroll>0);
        assert!(window.scroll<window.wrapped_desc.len());
        assert!(!log.get(900_002).unwrap().active,"reading must not change server journal state");
    }

    #[test]
    fn controller_selects_a_different_quest_and_opens_it_for_tracking() {
        let mut log=QuestLog::default();
        log.replace(vec![Quest{id:500,active:true,..Default::default()},Quest{id:501,active:true,..Default::default()}]);
        assert!(log.follow(500));
        let mut win=QuestWindow::new();win.toggle();
        let mut character=Character::new();let data=DataTable::new();
        let mut state=StateCache::new();let mut input=UiContext::new(800.0,600.0);
        input.pad_labels=true;input.key_down=true;input.key_enter=true;
        let mut ui=test_frame(&mut input,&mut state);
        let mut ctx=crate::BuildCtx::test(&mut character,&data);ctx.quest_log=&log;
        let events=win.build(&mut ui,&mut ctx);
        assert!(events.iter().any(|e|matches!(e,GameEvent::OpenQuestDetail{quest_id:501})));
        assert!(!win.is_open(),"the detail dialog must own the next pad press");
    }


    #[test]
    fn right_click_row_requests_active_toggle() {
        let mut log = QuestLog::default();
        log.replace(vec![Quest {
            id: 500,
            active: true,
            end_time: None,
            objectives: vec![QuestObjective {
                hunt_id: 500_000,
                mob_id: 1002,
                name: "Poring".into(),
                current: 1,
                required: 5,
            }],
        }]);

        let mut win = QuestWindow::new();
        win.toggle();

        let mut character = Character::new();
        let data = DataTable::new();
        let mut state = StateCache::new();
        let mut ctx = UiContext::new(800.0, 600.0);
        // First row, inside the list panel (below the 17px titlebar).
        ctx.mouse_x = TAB_STRIP_W + 40.0;
        ctx.mouse_y = DEFAULT_Y + TITLE_H + ROW_H / 2.0;
        ctx.mouse_right_clicked = true;
        let mut ui = test_frame(&mut ctx, &mut state);
        let mut build_ctx = crate::BuildCtx::test(&mut character, &data);
        build_ctx.quest_log = &log;
        let events = win.build(&mut ui, &mut build_ctx);

        assert!(
            events.iter().any(|e| matches!(
                e,
                GameEvent::RequestToggleQuestActive {
                    quest_id: 500,
                    active: false
                }
            )),
            "expected toggle-active event, got {events:?}"
        );
    }
}
