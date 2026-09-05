use leptos::*;
use crate::state::GameState;
use crate::components::icons::{Icon, IconKind};
use shared::{ClientRequest, GameRules, LobbyGameInfo, PlayerColour};

const BTN_PRIMARY: &str = "bg-orange-600 hover:bg-orange-500 text-white font-bold rounded-xl transition-all shadow-lg active:scale-95 disabled:opacity-50";

#[component]
pub fn MainMenu() -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");
    let (view_list, set_view_list) = create_signal(false);

    view! {
        <div class="flex flex-col items-center justify-center min-h-screen bg-slate-950 text-slate-200 p-4 font-sans">

            <Show when=move || state.connection_error.get().is_some()>
                <div class="mb-6 p-4 bg-red-900/30 border border-red-500/50 text-red-200 rounded-2xl text-sm max-w-md w-full animate-in fade-in duration-500">
                    <div class="flex items-center gap-3">
                        <span class="text-xl text-red-300"><Icon kind=IconKind::Warning /></span>
                        <div>
                            <p class="font-bold">"Connection Error"</p>
                            <p class="text-xs opacity-80">{move || state.connection_error.get()}</p>
                        </div>
                    </div>
                </div>
            </Show>

            <Show when=move || state.is_connecting.get()>
                <div class="mb-6 flex items-center gap-2 text-orange-500 animate-pulse">
                    <div class="w-2 h-2 bg-orange-500 rounded-full"></div>
                    "Connecting to server..."
                </div>
            </Show>

            <div class="p-10 bg-slate-900 rounded-3xl border border-slate-800 shadow-2xl text-center space-y-8 w-full max-w-md relative overflow-hidden">
                <div class="absolute -top-10 -right-10 w-32 h-32 bg-orange-600/10 rounded-full blur-3xl"></div>

                <h1 class="text-6xl font-black text-orange-600 italic tracking-tighter drop-shadow-sm">"COLONIST"</h1>

                <div class="text-left space-y-1">
                    <label class="text-slate-400 font-bold uppercase text-[10px] tracking-widest">"Your name"</label>
                    <input
                        class="w-full bg-slate-950 border border-slate-700 focus:border-orange-600 outline-none rounded-xl px-4 py-3 text-slate-100 placeholder:text-slate-600 transition-colors"
                        maxlength=16
                        placeholder="Leave blank for a default"
                        prop:value=move || state.my_name.get()
                        on:input=move |ev| state.my_name.set(event_target_value(&ev))
                    />
                </div>

                <Show
                    when=move || view_list.get()
                    fallback=move || view! { <ActionButtons set_view_list=set_view_list state=state /> }
                >
                    <div class="space-y-4 animate-in slide-in-from-right duration-300">
                        <div class="flex justify-between items-center mb-2">
                            <h2 class="text-slate-400 font-bold uppercase text-xs tracking-widest">"Active Lobbies"</h2>
                            <button
                                on:click=move |_| set_view_list.set(false)
                                class="text-slate-500 hover:text-white text-xs underline transition-colors"
                            >
                                "Back"
                            </button>
                        </div>

                        <div class="max-h-64 overflow-y-auto space-y-2 pr-2 custom-scrollbar min-h-[100px]">
                            <For
                                each=move || state.lobby_games.get()
                                key=|g| g.game_id.clone()
                                children=move |lobby| view! { <LobbyItem lobby=lobby state=state /> }
                            />

                            <Show when=move || state.lobby_games.get().is_empty()>
                                <div class="py-10 text-slate-600 italic text-sm">"No games found..."</div>
                            </Show>
                        </div>

                        <button
                            on:click=move |_| state.send(ClientRequest::GetLobbyList)
                            class="text-xs text-orange-500/50 hover:text-orange-500 transition-colors flex items-center justify-center gap-1 w-full"
                        >
                            <Icon kind=IconKind::Refresh class="mr-1" /> "Refresh list"
                        </button>
                    </div>
                </Show>
            </div>

            <p class="mt-8 text-slate-700 text-[10px] uppercase tracking-[0.3em]">"Powered by Rust & Leptos"</p>
        </div>
    }
}

#[component]
fn ActionButtons(set_view_list: WriteSignal<bool>, state: GameState) -> impl IntoView {
    let (creating, set_creating) = create_signal(false);
    let (table_size, set_table_size) = create_signal(4usize);

    view! {
        <div class="flex flex-col gap-4 animate-in zoom-in-95 duration-200">
            <button
                class=format!("{BTN_PRIMARY} py-4 text-xl")
                disabled=move || state.ws_sender.get().is_none()
                on:click=move |_| {
                    set_view_list.set(true);
                    state.send(ClientRequest::GetLobbyList);
                }
            >
                "BROWSE LOBBIES"
            </button>

            <Show
                when=move || creating.get()
                fallback=move || view! {
                    <button
                        class="border-2 border-slate-700 hover:border-orange-600 text-slate-400 hover:text-white py-4 rounded-xl font-bold transition-all disabled:opacity-20"
                        disabled=move || state.ws_sender.get().is_none()
                        on:click=move |_| set_creating.set(true)
                    >
                        "CREATE NEW GAME"
                    </button>
                }
            >
                <div class="space-y-4">
                    <div class="space-y-2">
                        <p class="text-slate-400 font-bold uppercase text-[10px] tracking-widest text-left">
                            "Table size"
                        </p>
                        <div class="grid grid-cols-5 gap-2">
                            {GameRules::SUPPORTED_PLAYER_COUNTS.into_iter().map(|n| {
                                view! {
                                    <button
                                        class=move || if table_size.get() == n {
                                            "py-2 rounded-lg font-bold text-sm bg-orange-600 text-white border-2 border-orange-500".to_string()
                                        } else {
                                            "py-2 rounded-lg font-bold text-sm bg-slate-950 text-slate-400 border-2 border-slate-800 hover:border-slate-600".to_string()
                                        }
                                        on:click=move |_| set_table_size.set(n)
                                    >
                                        {n}
                                    </button>
                                }
                            }).collect_view()}
                        </div>
                        <p class="text-[10px] text-slate-500 text-left">
                            {move || {
                                let rules = GameRules::for_player_count(table_size.get());
                                let board = if rules.board == shared::BoardLayout::Extended {
                                    "30-hex extension board"
                                } else {
                                    "19-hex base board"
                                };
                                format!(
                                    "First to {} points · {} · {} of each resource",
                                    rules.victory_points_to_win, board, rules.bank_per_resource,
                                )
                            }}
                        </p>
                    </div>

                    // A brand new game has the whole palette free.
                    <ColourPicker
                        state=state
                        available=PlayerColour::ALL.to_vec()
                        label="Pick your colour"
                        on_pick=Callback::new(move |colour: PlayerColour| {
                            state.send(ClientRequest::CreateGame {
                                player_count: table_size.get(),
                                seat: state.seat_request(colour),
                            });
                            set_creating.set(false);
                        })
                    />
                </div>
            </Show>
        </div>
    }
}

/// The palette. Only colours nobody in that game has taken are offered.
#[component]
fn ColourPicker(
    state: GameState,
    available: Vec<PlayerColour>,
    label: &'static str,
    on_pick: Callback<PlayerColour>,
) -> impl IntoView {
    let _ = state;

    view! {
        <div class="space-y-3 animate-in zoom-in-95 duration-200">
            <p class="text-slate-400 font-bold uppercase text-[10px] tracking-widest text-left">{label}</p>
            <div class="grid grid-cols-4 gap-3">
                {available.into_iter().map(|colour| {
                    view! {
                        <button
                            class="group flex flex-col items-center gap-1.5 rounded-xl border-2 border-slate-800 hover:border-white/70 p-2 transition-all active:scale-95"
                            title=colour.label()
                            on:click=move |_| on_pick.call(colour)
                        >
                            <span
                                class="w-8 h-8 rounded-full shadow-inner ring-2 ring-black/40"
                                style=format!("background-color: {}", colour.hex())
                            ></span>
                            <span class="text-[9px] uppercase tracking-wider text-slate-500 group-hover:text-slate-200">
                                {colour.label()}
                            </span>
                        </button>
                    }
                }).collect_view()}
            </div>
        </div>
    }
}

#[component]
fn LobbyItem(lobby: LobbyGameInfo, state: GameState) -> impl IntoView {
    let (picking, set_picking) = create_signal(false);

    let LobbyGameInfo { game_id, players, max_players, available_colours, victory_points_to_win } = lobby;
    let is_full = players >= max_players || available_colours.is_empty();
    let gid = game_id.clone();

    view! {
        <div class="p-4 bg-slate-950 border border-slate-800 rounded-2xl hover:border-orange-600/30 transition-all space-y-3">
            <div class="flex items-center justify-between">
                <div class="text-left">
                    <div class="font-mono text-orange-500 font-bold text-sm">"ID: " {game_id}</div>
                    <div class="text-[10px] text-slate-600 font-bold uppercase tracking-wider">
                        {players} " / " {max_players} " Players · First to " {victory_points_to_win}
                    </div>
                </div>

                <button
                    disabled=is_full || state.ws_sender.get().is_none()
                    class=move || if is_full {
                        "bg-slate-800 text-slate-600 px-4 py-2 rounded-xl text-xs font-bold cursor-not-allowed".to_string()
                    } else {
                        format!("{BTN_PRIMARY} px-4 py-2 text-xs")
                    }
                    on:click=move |_| set_picking.update(|p| *p = !*p)
                >
                    {if is_full { "FULL" } else { "JOIN" }}
                </button>
            </div>

            // Only the colours still free in *this* game are offered.
            <Show when=move || picking.get() && !is_full>
                <ColourPicker
                    state=state
                    available=available_colours.clone()
                    label="Pick a free colour"
                    on_pick=Callback::new({
                        let gid = gid.clone();
                        move |colour: PlayerColour| {
                            state.send(ClientRequest::JoinGame {
                                game_id: gid.clone(),
                                seat: state.seat_request(colour),
                            });
                            set_picking.set(false);
                        }
                    })
                />
            </Show>
        </div>
    }
}
