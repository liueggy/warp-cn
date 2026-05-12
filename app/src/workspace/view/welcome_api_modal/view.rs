//! Welcome modal shown on first launch of warp-cn.
//! Displays branding info about SSYCloud (胜算云) and guides users to configure
//! their AI API key in settings.

use markdown_parser::{
    FormattedText, FormattedTextFragment, FormattedTextLine, FormattedTextStyles, Hyperlink,
};
use pathfinder_color::ColorU;
use pathfinder_geometry::vector::vec2f;
use warp_core::ui::theme::{phenomenon::PhenomenonStyle, Fill};
use warpui::elements::{
    Align, ChildAnchor, ChildView, ConstrainedBox, Container, CornerRadius, CrossAxisAlignment, Flex,
    FormattedTextElement, HighlightedHyperlink, MainAxisSize, OffsetPositioning, ParentAnchor,
    ParentElement, ParentOffsetBounds, Radius, Stack, Text,
};
use warpui::fonts::{Properties, Weight};
use warpui::keymap::FixedBinding;
use warpui::{
    AppContext, Element, Entity, SingletonEntity, TypedActionView, View, ViewContext, ViewHandle,
};

use crate::appearance::Appearance;
use crate::ui_components::icons::Icon;
use crate::view_components::action_button::{ActionButton, ActionButtonTheme, ButtonSize};
use crate::workspace::action::WorkspaceAction;
use crate::settings_view::SettingsSection;

const MODAL_WIDTH: f32 = 460.;
const SSY_URL: &str = "https://www.shengsuanyun.com/?from=CH_3G9WAVFJ";

pub fn init(app: &mut AppContext) {
    use warpui::keymap::macros::*;

    app.register_fixed_bindings([FixedBinding::new(
        "escape",
        WelcomeApiModalAction::Close,
        id!(WelcomeApiModal::ui_name()),
    )]);
}

#[derive(Clone, Debug)]
pub enum WelcomeApiModalAction {
    Close,
    GoToSettings,
}

#[derive(Clone, Debug)]
pub enum WelcomeApiModalEvent {
    Close,
    GoToSettings,
}

struct CloseButtonTheme;

impl ActionButtonTheme for CloseButtonTheme {
    fn background(&self, hovered: bool, _appearance: &Appearance) -> Option<Fill> {
        if hovered {
            Some(Fill::Solid(PhenomenonStyle::modal_close_button_hover()))
        } else {
            None
        }
    }

    fn text_color(
        &self,
        _hovered: bool,
        _background: Option<Fill>,
        _appearance: &Appearance,
    ) -> ColorU {
        PhenomenonStyle::modal_close_button_text()
    }
}

struct CtaButtonTheme;

impl ActionButtonTheme for CtaButtonTheme {
    fn background(&self, hovered: bool, _appearance: &Appearance) -> Option<Fill> {
        Some(PhenomenonStyle::modal_button_background_fill(hovered))
    }

    fn text_color(
        &self,
        _hovered: bool,
        _background: Option<Fill>,
        _appearance: &Appearance,
    ) -> ColorU {
        PhenomenonStyle::modal_button_text()
    }
}

pub struct WelcomeApiModal {
    close_button: ViewHandle<ActionButton>,
    cta_button: ViewHandle<ActionButton>,
}

impl WelcomeApiModal {
    pub fn new(ctx: &mut ViewContext<Self>) -> Self {
        let close_button = ctx.add_view(|_ctx| {
            ActionButton::new("", CloseButtonTheme)
                .with_icon(Icon::X)
                .with_size(ButtonSize::Small)
                .on_click(|ctx| ctx.dispatch_typed_action(WelcomeApiModalAction::Close))
        });

        let cta_button = ctx.add_view(|_ctx| {
            ActionButton::new("去配置", CtaButtonTheme)
                .with_full_width(true)
                .on_click(|ctx| ctx.dispatch_typed_action(WelcomeApiModalAction::GoToSettings))
        });

        Self {
            close_button,
            cta_button,
        }
    }

    fn render_header(&self, appearance: &Appearance) -> Box<dyn Element> {
        let icon_el = ConstrainedBox::new(
            Icon::Stars
                .to_warpui_icon(Fill::Solid(PhenomenonStyle::modal_badge_text()))
                .finish(),
        )
        .with_width(20.)
        .with_height(20.)
        .finish();

        let title = Text::new("欢迎使用 Warp-CN", appearance.ui_font_family(), 20.)
            .with_color(PhenomenonStyle::modal_title_text())
            .with_style(Properties::default().weight(Weight::Semibold))
            .finish();

        let close_el = Container::new(ChildView::new(&self.close_button).finish())
            .with_uniform_padding(4.)
            .with_padding_right(2.)
            .finish();

        let mut header_stack = Stack::new();
        header_stack.add_child(
            Flex::row()
                .with_cross_axis_alignment(CrossAxisAlignment::Center)
                .with_spacing(8.)
                .with_child(icon_el)
                .with_child(title)
                .finish(),
        );
        header_stack.add_positioned_child(
            close_el,
            OffsetPositioning::offset_from_parent(
                vec2f(-4., 0.),
                ParentOffsetBounds::ParentByPosition,
                ParentAnchor::TopRight,
                ChildAnchor::TopRight,
            ),
        );
        header_stack.finish()
    }

    fn render_description(appearance: &Appearance) -> Box<dyn Element> {
        let ssy_link = FormattedTextFragment {
            text: "胜算云".into(),
            styles: FormattedTextStyles {
                underline: true,
                hyperlink: Some(Hyperlink::Url(SSY_URL.into())),
                ..Default::default()
            },
        };

        let register_link = FormattedTextFragment {
            text: "立即注册获10元模力及首充10%赠送".into(),
            styles: FormattedTextStyles {
                underline: true,
                hyperlink: Some(Hyperlink::Url(SSY_URL.into())),
                ..Default::default()
            },
        };

        let fragments = vec![
            FormattedTextFragment::plain_text("Warp-CN 由"),
            ssy_link,
            FormattedTextFragment::plain_text("基于开源项目 Warp 汉化。"),
            FormattedTextFragment::plain_text("胜算云是专为 AI Native Teams 服务的超级工厂，工业级 AI 任务并行执行平台。模型商城集采直供聚合接入了 Claude、ChatGPT、Gemini 等海内外 LLM 及图片视频多媒体模型算力，绝无逆向掺水、全站模型 SLA 可用性高达 99.7%。更有企业级专属定制网关，实现团队精细化成本与权限管控，智能路由+安全防护+BYOK 企业自带密钥托管。"),
            register_link,
        ];

        let formatted = FormattedText::new([FormattedTextLine::Line(fragments)]);

        FormattedTextElement::new(
            formatted,
            14.,
            appearance.ui_font_family(),
            appearance.monospace_font_family(),
            PhenomenonStyle::modal_feature_description_text(),
            HighlightedHyperlink::default(),
        )
        .with_line_height_ratio(1.5)
        .with_hyperlink_font_color(appearance.theme().accent().into_solid())
        .register_default_click_handlers(|link, _ctx, app| {
            app.open_url(&link.url);
        })
        .finish()
    }

    fn render_body(&self, appearance: &Appearance) -> Box<dyn Element> {
        let cta = ChildView::new(&self.cta_button).finish();

        Container::new(
            Flex::column()
                .with_cross_axis_alignment(CrossAxisAlignment::Start)
                .with_main_axis_size(MainAxisSize::Min)
                .with_child(Self::render_description(appearance))
                .with_child(Container::new(cta).with_margin_top(24.).finish())
                .finish(),
        )
        .with_horizontal_padding(32.)
        .with_vertical_padding(24.)
        .with_background(Fill::Solid(PhenomenonStyle::modal_background()))
        .with_corner_radius(CornerRadius::with_bottom(Radius::Pixels(8.)))
        .finish()
    }
}

impl Entity for WelcomeApiModal {
    type Event = WelcomeApiModalEvent;
}

impl View for WelcomeApiModal {
    fn ui_name() -> &'static str {
        "WelcomeApiModal"
    }

    fn on_focus(&mut self, _focus_ctx: &warpui::FocusContext, ctx: &mut ViewContext<Self>) {
        ctx.focus_self();
    }

    fn render(&self, app: &AppContext) -> Box<dyn Element> {
        let appearance = Appearance::as_ref(app);

        let header = Container::new(self.render_header(appearance))
            .with_horizontal_padding(32.)
            .with_padding_top(24.)
            .with_padding_bottom(12.)
            .with_background(Fill::Solid(PhenomenonStyle::modal_background()))
            .with_corner_radius(CornerRadius::with_top(Radius::Pixels(8.)))
            .finish();

        let card = ConstrainedBox::new(
            Container::new(
                Flex::column()
                    .with_main_axis_size(MainAxisSize::Min)
                    .with_cross_axis_alignment(CrossAxisAlignment::Stretch)
                    .with_child(header)
                    .with_child(self.render_body(appearance))
                    .finish(),
            )
            .with_background(Fill::Solid(PhenomenonStyle::modal_background()))
            .with_corner_radius(CornerRadius::with_all(Radius::Pixels(8.)))
            .finish(),
        )
        .with_width(MODAL_WIDTH)
        .finish();

        Container::new(Align::new(card).finish())
            .with_background_color(ColorU::new(18, 18, 18, 128))
            .finish()
    }
}

impl TypedActionView for WelcomeApiModal {
    type Action = WelcomeApiModalAction;

    fn handle_action(&mut self, action: &Self::Action, ctx: &mut ViewContext<Self>) {
        match action {
            WelcomeApiModalAction::Close => {
                ctx.emit(WelcomeApiModalEvent::Close);
            }
            WelcomeApiModalAction::GoToSettings => {
                ctx.emit(WelcomeApiModalEvent::GoToSettings);
            }
        }
    }
}
