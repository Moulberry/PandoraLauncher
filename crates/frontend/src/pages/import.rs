use bridge::{handle::BackendHandle, import::{ImportFromOtherLaunchers, OtherLauncher}, message::{MessageToBackend, SyncState}, modal_action::ModalAction};
use enumset::EnumSet;
use gpui::{prelude::*, *};
use gpui_component::{
    button::{Button, ButtonVariants}, checkbox::Checkbox, h_flex, scroll::ScrollableElement, spinner::Spinner, tooltip::Tooltip, v_flex, ActiveTheme as _, Disableable, Icon, IconName, Sizable
};

use crate::{component::{page::Page, responsive_grid::ResponsiveGrid}, entity::DataEntities, ui};

pub struct ImportPage {
    backend_handle: BackendHandle,
    import_from_other_launchers: Option<ImportFromOtherLaunchers>,
    import_from: Option<OtherLauncher>,
    import_accounts: bool,
    import_instances: bool,
    _get_import_paths_task: Task<()>,
}

impl ImportPage {
    pub fn new(data: &DataEntities, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut page = Self {
            backend_handle: data.backend_handle.clone(),
            import_from_other_launchers: None,
            import_from: None,
            import_accounts: true,
            import_instances: true,
            _get_import_paths_task: Task::ready(()),
        };

        page.update_launcher_paths(cx);

        page
    }

    pub fn update_launcher_paths(&mut self, cx: &mut Context<Self>) {
        let (send, recv) = tokio::sync::oneshot::channel();
        self._get_import_paths_task = cx.spawn(async move |page, cx| {
            let result: ImportFromOtherLaunchers = recv.await.unwrap_or_default();
            let _ = page.update(cx, move |page, cx| {
                page.import_from_other_launchers = Some(result);
                cx.notify();
            });
        });

        self.backend_handle.send(MessageToBackend::GetImportFromOtherLauncherPaths {
            channel: send,
        });
    }
}

impl Render for ImportPage {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(imports) = &self.import_from_other_launchers else {
            let content = v_flex().size_full().p_3().gap_3()
                .child(Spinner::new().with_size(gpui_component::Size::Large));

            return Page::new("Import")
                .scrollable()
                .child(content);
        };

        let mut content = v_flex().size_full().p_3().gap_3()
            .child(ResponsiveGrid::new(Size::new(AvailableSpace::MinContent, AvailableSpace::MinContent))
                .gap_2()
                .child(Button::new("prism")
                    .label("Import from Prism")
                    .w_full()
                    .disabled(imports.imports[OtherLauncher::Prism].is_none())
                    .on_click(cx.listener(|page, _, _, _| page.import_from = Some(OtherLauncher::Prism))))
                .child(Button::new("modrinth")
                    .label("Import from Modrinth")
                    .w_full()
                    .disabled(imports.imports[OtherLauncher::Modrinth].is_none())
                    .on_click(cx.listener(|page, _, _, _| page.import_from = Some(OtherLauncher::Modrinth))))
                .child(Button::new("mmc")
                    .label("Import from MultiMC")
                    .w_full()
                    .disabled(imports.imports[OtherLauncher::MultiMC].is_none())
                    .on_click(cx.listener(|page, _, _, _| page.import_from = Some(OtherLauncher::MultiMC))))
            );

        if let Some(import_from) = self.import_from && let Some(import) = &imports.imports[import_from] {
            let label = match import_from {
                OtherLauncher::Prism => "Import From Prism",
                OtherLauncher::Modrinth => "Import From Modrinth",
                OtherLauncher::MultiMC => "Import From MultiMC",
            };
            let import_accounts = self.import_accounts && import.can_import_accounts;
            content = content.child(v_flex()
                .w_full()
                .border_1()
                .gap_2()
                .p_2()
                .rounded(cx.theme().radius_lg)
                .border_color(cx.theme().border)
                .when(import.can_import_accounts, |div| div.child(Checkbox::new("accounts").label("Import Accounts")
                    .checked(self.import_accounts)
                    .on_click(cx.listener(|page, checked, _, _| {
                    page.import_accounts = *checked;
                }))))
                .child(Checkbox::new("instances").label("Import Instances")
                    .checked(self.import_instances)
                    .on_click(cx.listener(|page, checked, _, _| {
                    page.import_instances = *checked;
                })))
                .when(self.import_instances, |d| d.child(div()
                    .w_full()
                    .border_1()
                    .p_2()
                    .rounded(cx.theme().radius)
                    .border_color(cx.theme().border)
                    .max_h_64()
                    .child(v_flex().overflow_y_scrollbar().children(
                        import.paths.iter().map(|path| {
                            SharedString::new(path.to_string_lossy())
                        })
                    )))
                )
                .child(Button::new("doimport").disabled(!import_accounts && !self.import_instances).success().label(label).on_click(cx.listener(move |page, _, window, cx| {
                    let modal_action = ModalAction::default();

                    page.backend_handle.send(MessageToBackend::ImportFromOtherLauncher {
                        launcher: import_from,
                        import_accounts: import_accounts,
                        import_instances: page.import_instances,
                        modal_action: modal_action.clone()
                    });

                    let title = SharedString::new_static(label);
                    crate::modals::generic::show_modal(window, cx, title, "Error importing".into(), modal_action);
                })))
            )
        }

        Page::new("Import")
            .scrollable()
            .child(content)
    }
}
