use std::time::Instant;

use gpui::{prelude::*, *};
use gpui_component::{Selectable, Sizable, button::Button, h_flex, slider::{Slider, SliderEvent, SliderState}, v_flex};
use schema::{minecraft_profile::SkinVariant, unique_bytes::UniqueBytes};

use crate::{component::player_model::{self, PlayerModel, PlayerModelState}, icon::PandoraIcon};

pub struct PlayerModelWidget {
    player_model_state: Entity<PlayerModelState>,
    yaw_slider_state: Entity<SliderState>,
    pitch_slider_state: Entity<SliderState>,
    animation_slider_state: Entity<SliderState>,
    animating_yaw: bool,
    animating_pitch_positive: bool,
    animating_pitch: bool,
    animating_animation: bool,
    variant: SkinVariant,
    last_drag: Option<Point<Pixels>>,
    last_render: Instant,
}

impl PlayerModelWidget {
    pub fn new(cx: &mut Context<Self>, skin: UniqueBytes) -> Self {
        let yaw_slider_state = cx.new(|_| {
            SliderState::new().min(-180.0).max(180.0).default_value(player_model::DEFAULT_YAW as f32)
        });
        let pitch_slider_state = cx.new(|_| {
            SliderState::new().min(-90.0).max(90.0).default_value(player_model::DEFAULT_PITCH as f32)
        });
        let animation_slider_state = cx.new(|_| {
            SliderState::new().min(0.0).max(1.0).step(1.0/800.0).default_value(player_model::DEFAULT_ANIMATION as f32)
        });

        let variant = crate::skin_renderer::determine_skin_variant(&skin).unwrap_or(SkinVariant::Classic);

        cx.subscribe(&yaw_slider_state, Self::on_yaw_changed).detach();
        cx.subscribe(&pitch_slider_state, Self::on_pitch_changed).detach();
        cx.subscribe(&animation_slider_state, Self::on_animation_changed).detach();

        Self {
            player_model_state: PlayerModelState::new(cx, skin, variant),
            yaw_slider_state,
            pitch_slider_state,
            animation_slider_state,
            animating_yaw: false,
            animating_pitch_positive: true,
            animating_pitch: false,
            animating_animation: false,
            variant,
            last_drag: None,
            last_render: Instant::now(),
        }
    }

    pub fn set_skin(&mut self, cx: &mut App, skin: UniqueBytes, variant: SkinVariant) {
        self.variant = variant;
        let mut state = self.player_model_state.as_mut(cx);
        state.skin = skin;
        state.variant = variant;
    }

    pub fn set_cape(&mut self, cx: &mut App, cape: Option<UniqueBytes>) {
        self.player_model_state.as_mut(cx).cape = cape;
    }

    pub fn set_variant(&mut self, cx: &mut App, variant: SkinVariant) {
        self.variant = variant;
        self.player_model_state.as_mut(cx).variant = variant;
    }

    pub fn get_variant(&self) -> SkinVariant {
        self.variant
    }

    pub fn set_skin_and_cape(&mut self, cx: &mut App, skin: UniqueBytes, variant: SkinVariant, cape: Option<UniqueBytes>) {
        self.variant = variant;
        let mut state = self.player_model_state.as_mut(cx);
        state.skin = skin;
        state.variant = variant;
        state.cape = cape;
    }

    fn on_yaw_changed(&mut self, _: Entity<SliderState>, event: &SliderEvent, cx: &mut Context<Self>) {
        let SliderEvent::Change(change) = event;
        self.animating_yaw = false;
        self.player_model_state.update(cx, |state, cx| {
            state.yaw = change.start() as f64;
            cx.notify();
        })
    }

    fn on_pitch_changed(&mut self, _: Entity<SliderState>, event: &SliderEvent, cx: &mut Context<Self>) {
        let SliderEvent::Change(change) = event;
        self.animating_pitch = false;
        self.player_model_state.update(cx, |state, cx| {
            state.pitch = change.start() as f64;
            cx.notify();
        })
    }

    fn on_animation_changed(&mut self, _: Entity<SliderState>, event: &SliderEvent, cx: &mut Context<Self>) {
        let SliderEvent::Change(change) = event;
        self.animating_animation = false;
        self.player_model_state.update(cx, |state, cx| {
            state.animation = change.start() as f64;
            cx.notify();
        })
    }

    pub fn update_animations(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.animating_yaw && !self.animating_pitch && !self.animating_animation {
            return;
        }

        let now = Instant::now();
        let delta = now - self.last_render;
        self.player_model_state.update(cx, |state, cx| {
            if self.animating_yaw {
                state.yaw += delta.as_secs_f64()*360.0/8.0;
                state.yaw %= 360.0;
                if state.yaw < -180.0 {
                    state.yaw += 360.0;
                }
                if state.yaw > 180.0 {
                    state.yaw -= 360.0;
                }
                self.yaw_slider_state.update(cx, |slider, cx| {
                    slider.set_value(state.yaw as f32, window, cx)
                });
            }
            if self.animating_pitch {
                if self.animating_pitch_positive {
                    state.pitch += delta.as_secs_f64()*180.0/8.0;
                } else {
                    state.pitch -= delta.as_secs_f64()*180.0/8.0;
                }
                if state.pitch > 90.0 {
                    state.pitch = 90.0;
                    self.animating_pitch_positive = false;
                }
                if state.pitch < -90.0 {
                    state.pitch = -90.0;
                    self.animating_pitch_positive = true;
                }
                self.pitch_slider_state.update(cx, |slider, cx| {
                    slider.set_value(state.pitch as f32, window, cx)
                });
            }
            if self.animating_animation {
                state.animation += delta.as_secs_f64()/8.0;
                state.animation %= 1.0;
                self.animation_slider_state.update(cx, |slider, cx| {
                    slider.set_value(state.animation as f32, window, cx)
                });
            }
        });

        self.last_render = now;
        window.request_animation_frame();
    }
}

#[derive(Clone, Copy)]
struct RotatingModel;

impl Render for RotatingModel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        gpui::Empty
    }
}

impl Render for PlayerModelWidget {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (yaw, pitch) = {
            let model_state = self.player_model_state.read(cx);
            (model_state.yaw, model_state.pitch)
        };

        self.update_animations(window, cx);

        v_flex()
            .h_full()
            .child(v_flex()
                .size_full()
                .id("player_model_widget")
                .child(PlayerModel::new(&self.player_model_state))
                .cursor_grab()
                .on_mouse_up(MouseButton::Left, cx.listener(|widget, _: &MouseUpEvent, _, _| {
                    widget.last_drag = None;
                }))
                .on_mouse_up_out(MouseButton::Left, cx.listener(|widget, _: &MouseUpEvent, _, _| {
                    widget.last_drag = None;
                }))
                .on_drag(RotatingModel, |_, _, _, cx| {
                    cx.new(|_| RotatingModel)
                })
                .on_drag_move(cx.listener({
                    |widget, event: &DragMoveEvent<RotatingModel>, window, cx| {
                        if cx.active_drag_cursor_style() != Some(CursorStyle::ClosedHand) {
                            cx.set_active_drag_cursor_style(CursorStyle::ClosedHand, window);
                        }
                        if let Some(point) = widget.last_drag {
                            widget.player_model_state.update(cx, |state, cx| {
                                state.yaw += (event.event.position.x.to_f64() - point.x.to_f64()) * 0.5;
                                state.yaw %= 360.0;
                                if state.yaw < -180.0 {
                                    state.yaw += 360.0;
                                }
                                if state.yaw > 180.0 {
                                    state.yaw -= 360.0;
                                }
                                state.pitch += (event.event.position.y.to_f64() - point.y.to_f64()) * 0.5;
                                state.pitch = state.pitch.clamp(-90.0, 90.0);
                                widget.yaw_slider_state.update(cx, |slider, cx| {
                                    slider.set_value(state.yaw as f32, window, cx)
                                });
                                widget.pitch_slider_state.update(cx, |slider, cx| {
                                    slider.set_value(state.pitch as f32, window, cx)
                                });
                                cx.notify();
                            });
                        }
                        widget.last_drag = Some(event.event.position);
                    }
                }))
            )
            .child(v_flex().p_4().w_full()
                .child(h_flex()
                    .w_full()
                    .gap_2()
                    .pb_2()
                    .child(Button::new("classic")
                        .label("Classic")
                        .flex_1()
                        .selected(self.variant == SkinVariant::Classic)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.variant = SkinVariant::Classic;
                            this.player_model_state.update(cx, |state, _| {
                                state.variant = SkinVariant::Classic;
                            });
                        })))
                    .child(Button::new("slim")
                        .label("Slim")
                        .flex_1()
                        .selected(self.variant == SkinVariant::Slim)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.variant = SkinVariant::Slim;
                            this.player_model_state.update(cx, |state, _| {
                                state.variant = SkinVariant::Slim;
                            });
                        }))))
                .child(v_flex()
                    .child(h_flex().w_full().justify_between().text_sm()
                        .child(format!("Yaw: {}°", yaw as i32))
                        .child(Button::new("play-yaw").compact().small()
                            .icon(PandoraIcon::pause_play(self.animating_yaw))
                            .on_click(cx.listener(|widget, _, _, cx| {
                                widget.animating_yaw = !widget.animating_yaw;
                                widget.last_render = Instant::now();
                                cx.notify();
                            }))))
                    .child(Slider::new(&self.yaw_slider_state)))
                .child(v_flex()
                    .child(h_flex().w_full().justify_between().text_sm()
                        .child(format!("Pitch: {}°", pitch as i32))
                        .child(Button::new("play-pitch").compact().small()
                            .icon(PandoraIcon::pause_play(self.animating_pitch))
                            .on_click(cx.listener(|widget, _, _, cx| {
                                widget.animating_pitch = !widget.animating_pitch;
                                widget.last_render = Instant::now();
                                cx.notify();
                            }))))
                    .child(Slider::new(&self.pitch_slider_state)))
                .child(v_flex()
                    .child(h_flex().w_full().justify_between().text_sm()
                        .child("Animation")
                        .child(Button::new("play-anim").compact().small()
                            .icon(PandoraIcon::pause_play(self.animating_animation))
                            .on_click(cx.listener(|widget, _, _, cx| {
                                widget.animating_animation = !widget.animating_animation;
                                widget.last_render = Instant::now();
                                cx.notify();
                            }))))
                    .child(Slider::new(&self.animation_slider_state)))
            )
    }
}
