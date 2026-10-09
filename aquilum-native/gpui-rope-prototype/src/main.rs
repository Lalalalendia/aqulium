//! Native Aquilum editor with a bounded GPUI multiline viewport on top of
//! a Rope document model, backed by the REAL production DocumentHub/Yrs.
//!
//! This experiment is deliberately NOT a production replacement. It does not
//! implement reliable crash-recovery, undo synchronization, Markdown live
//! preview, accessibility parity, or a safe exit dialog. TEST FILE COPIES ONLY.
//!
//! No whole 1/5/20 MiB document is inserted into GPUI InputState: each tab has
//! an independent 8192-character window. Save generates a compact change,
//! validates UTF-16 offsets, pushes to DocumentHub first, then commits Rope.

use aquilum_native_common::{lab_file_paths, lab_profile_dir, NativeDocument, NativeHost};
use aquilum_native_rope_viewport::{TextModel, Viewport};
use gpui::{
    div, prelude::*, px, rgb, size, App, Application, Bounds, Context,
    Entity, Subscription, Window, WindowBounds, WindowOptions,
};
use gpui_component::{input::{Input, InputEvent, InputState}, Root};

const VISIBLE_CHARS: usize = 8_192;
const MAX_EDITED_CHARS: usize = 16_384;

struct Tab {
    document: NativeDocument,
    model: TextModel,
    editor: Entity<InputState>,
    view: Viewport,
    dirty: bool,
}

struct NativeRopeEditor {
    _host: NativeHost,
    tabs: Vec<Tab>,
    selected: usize,
    status: String,
    _listeners: Vec<Subscription>,
}
impl NativeRopeEditor {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let paths = lab_file_paths().expect("Usage: aquilum-native-gpui-rope --file PATH [--file PATH]");
        let host = NativeHost::new(&lab_profile_dir()).expect("Cannot open actual Aquilum core");
        let mut tabs=Vec::new();
        let mut listeners=Vec::new();
        for path in paths {
            let mut document=host.open(&path).expect("Cannot read requested Markdown test file");
            let content=document.take_initial_text();
            let model=TextModel::from_str(&content);
            // Drop the String: the CRDT and Rope own the document now.
            drop(content);
            let view=model.window_at_char(0,VISIBLE_CHARS).expect("valid first viewport");
            let value=view.text.clone();
            let index=tabs.len();
            let editor=cx.new(|cx| InputState::new(window,cx).multi_line().default_value(value));
            let listener=cx.subscribe(&editor,move |this,_,event:&InputEvent,cx|{
                if matches!(event,InputEvent::Change) {
                    if let Some(tab)=this.tabs.get_mut(index) {
                        let actual=tab.editor.read(cx).value();
                        tab.dirty=actual.as_ref() != tab.view.text.as_str();
                        cx.notify();
                    }
                }
            });
            listeners.push(listener);
            tabs.push(Tab{document,model,editor,view,dirty:false});
        }
        Self {
            _host:host, tabs, selected:0, _listeners:listeners,
            status:format!("Rope virtual viewport {} Unicode chars; explicit Save only",VISIBLE_CHARS),
        }
    }

    fn install_window(&mut self, at:usize, window:&mut Window, cx:&mut Context<Self>) {
        let tab=&mut self.tabs[self.selected];
        let target=at.min(tab.model.len_chars());
        match tab.model.window_at_char(target,VISIBLE_CHARS) {
            Ok(view)=>{
                let content=view.text.clone();
                // A widget never sees more than VISIBLE_CHARS scalars.
                tab.editor.update(cx,|input,cx|input.set_value(content,window,cx));
                tab.view=view;
                tab.dirty=false;
                self.status=format!(
                    "Window {}..{} of {} scalars; document {} bytes; version {}",
                    tab.view.start_char,tab.view.end_char,tab.model.len_chars(),
                    tab.model.len_bytes(),tab.document.version(),
                );
            },
            Err(error)=>self.status=format!("Viewport error: {error:?}"),
        }
    }

    fn navigate(&mut self,forward:bool,window:&mut Window,cx:&mut Context<Self>) {
        let tab=&self.tabs[self.selected];
        if tab.dirty {
            self.status="Unsaved viewport edits: Save first. Navigation was refused.".into();
            return;
        }
        let next=if forward {
            tab.view.end_char
        } else {
            tab.view.start_char.saturating_sub(VISIBLE_CHARS)
        };
        self.install_window(next,window,cx);
    }

    fn switch_tab(&mut self,index:usize) {
        if index>=self.tabs.len(){return}
        if self.tabs[self.selected].dirty {
            self.status="Save or discard unsaved viewport edits before changing tab".into();
            return;
        }
        self.selected=index;
        self.status=format!(
            "Tab {}: showing {}..{} of {} chars",
            index+1,self.tabs[index].view.start_char,self.tabs[index].view.end_char,
            self.tabs[index].model.len_chars()
        );
    }

    fn save(&mut self,window:&mut Window,cx:&mut Context<Self>) {
        let tab=&mut self.tabs[self.selected];
        let edited=tab.editor.read(cx).value().to_string();
        let plan=match tab.model.propose_edited_viewport_text(
            &tab.view,&edited,MAX_EDITED_CHARS
        ){
            Ok(Some(plan))=>plan,
            Ok(None)=>{
                tab.dirty=false;
                self.status="No text changed in this window".into();
                return;
            },
            Err(error)=>{
                self.status=format!("Save blocked; viewport edit invalid: {error:?}");
                return;
            },
        };

        // This is the only mutation of the actual user-selected Markdown file:
        // a version-checked incremental Yrs edit through the existing Hub.
        let json=plan.to_changeset_json();
        let version=match tab.document.push_json_change(json){
            Ok(version)=>version,
            Err(error)=>{
                self.status=format!("Save REJECTED; local text retained: {error}");
                return;
            },
        };
        if let Err(error)=tab.model.commit_proposed_edit(plan) {
            self.status=format!(
                "Hub accepted version {version}, but Rope cache failed ({error:?}); refresh needed"
            );
            return;
        }

        let start=tab.view.start_char.min(tab.model.len_chars());
        let new_view=tab.model.window_at_char(start,VISIBLE_CHARS)
            .expect("valid view following accepted edit");
        let content=new_view.text.clone();
        tab.editor.update(cx,|input,cx|input.set_value(content,window,cx));
        tab.view=new_view;
        tab.dirty=false;
        self.status=format!(
            "Saved delta to Yrs version {version}, Rope rev {}; {} document bytes",
            tab.model.revision(),tab.model.len_bytes()
        );
    }

    fn reload(&mut self,window:&mut Window,cx:&mut Context<Self>) {
        let tab=&mut self.tabs[self.selected];
        if tab.dirty {
            self.status="Unsaved edits: reload refused".into();
            return;
        }
        let start=tab.view.start_char;
        match tab.document.refresh() {
            Ok(content)=>{
                tab.model=TextModel::from_str(&content);
                drop(content);
                self.install_window(start,window,cx);
            },
            Err(error)=>self.status=format!("DocumentHub refresh failed: {error}"),
        }
    }
}

impl Render for NativeRopeEditor {
    fn render(&mut self,_:&mut Window,cx:&mut Context<Self>)->impl IntoElement {
        let mut tabs=div().flex().gap_2();
        for index in 0..self.tabs.len() {
            let tab=&self.tabs[index];
            let label=format!(
                "{}{}",
                tab.document.path().file_name().unwrap_or_default().to_string_lossy(),
                if tab.dirty {" *"} else {""}
            );
            tabs=tabs.child(div().id(index).p_2().bg(rgb(0x354158)).cursor_pointer()
                .on_click(cx.listener(move |this,_,_,cx|{
                    this.switch_tab(index);
                    cx.notify();
                })).child(label));
        }
        let selected=&self.tabs[self.selected];
        let input=selected.editor.clone();
        div().flex().flex_col().size_full().bg(rgb(0x171d29))
            .text_color(rgb(0xe4e9f4)).p_3().gap_2()
            .child(div().text_xl().child("Aquilum Native: GPUI + Rope (bounded editing)"))
            .child(tabs)
            .child(div().flex().gap_2()
                .child(div().id("previous-page").p_2().bg(rgb(0x384d6e)).cursor_pointer()
                    .on_click(cx.listener(|this,_,window,cx|{
                        this.navigate(false,window,cx); cx.notify();
                    })).child("Previous 8192 chars"))
                .child(div().id("next-page").p_2().bg(rgb(0x384d6e)).cursor_pointer()
                    .on_click(cx.listener(|this,_,window,cx|{
                        this.navigate(true,window,cx); cx.notify();
                    })).child("Next 8192 chars"))
                .child(div().id("save-window").p_2().bg(rgb(0x386242)).cursor_pointer()
                    .on_click(cx.listener(|this,_,window,cx|{
                        this.save(window,cx); cx.notify();
                    })).child("Save delta"))
                .child(div().id("reload-window").p_2().bg(rgb(0x4d516c)).cursor_pointer()
                    .on_click(cx.listener(|this,_,window,cx|{
                        this.reload(window,cx); cx.notify();
                    })).child("Reload if clean"))
            )
            .child(div().text_sm().child(self.status.clone()))
            .child(div().text_sm().child(format!(
                "Full note {} bytes, {} lines; widget holds {} chars",
                selected.model.len_bytes(),selected.model.len_lines(),selected.view.text.chars().count()
            )))
            .child(div().id("bounded-viewport").flex_1().min_h(px(220.0))
                .child(Input::new(&input).h_full()))
    }
}

fn main(){
    Application::new().run(|cx:&mut App|{
        gpui_component::init(cx);
        let bounds=Bounds::centered(None,size(px(1120.0),px(750.0)),cx);
        cx.open_window(
            WindowOptions{
                window_bounds:Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window,cx|{
                let view=cx.new(|cx|NativeRopeEditor::new(window,cx));
                cx.new(|cx|Root::new(view.into(),window,cx))
            }
        ).expect("Unable to open native GPUI/Rope window");
        cx.activate(true);
    });
}
