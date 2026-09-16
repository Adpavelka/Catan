use leptos::*;
use crate::state::GameState;
use crate::components::icons::{Art, Icon, IconKind};
use shared::{ClientRequest, GameRules, LobbyGameInfo, PlayerColour};

const BTN_PRIMARY: &str = "bg-[#56c6e1] hover:bg-[#83dff0] text-[#06384a] font-black rounded-xl transition-all shadow-lg shadow-[#021e2d]/30 active:scale-[0.98] disabled:opacity-40 disabled:pointer-events-none";
const FIELD: &str = "w-full bg-white/95 border border-[#9ed9e8] focus:border-[#1689aa] outline-none rounded-xl px-4 py-3 text-[#123c4d] placeholder:text-[#7b9eaa] transition-colors";

#[component]
pub fn MainMenu() -> impl IntoView {
    let state = use_context::<GameState>().expect("GameState missing");
    let (view_list, set_view_list) = create_signal(false);

    view! {
        <div class="relative isolate flex flex-col items-center justify-center min-h-screen overflow-hidden bg-[radial-gradient(circle_at_top,#bcecf5_0%,#419dbb_42%,#075477_100%)] text-[#123c4d] p-4 font-sans">
            <div class="pointer-events-none absolute inset-0 z-0 opacity-80">
                <Art name="wood" alt="" class="absolute left-[5%] top-[14%] h-24 w-24 -rotate-12 object-contain drop-shadow-xl" />
                <Art name="sheep" alt="" class="absolute right-[8%] top-[17%] h-28 w-28 rotate-12 object-contain drop-shadow-xl" />
                <Art name="brick" alt="" class="absolute bottom-[14%] left-[10%] h-24 w-24 rotate-6 object-contain drop-shadow-xl" />
                <Art name="ore" alt="" class="absolute bottom-[10%] right-[12%] h-28 w-28 -rotate-12 object-contain drop-shadow-xl" />
                <Art name="wheat" alt="" class="absolute left-[7%] top-[48%] h-20 w-20 rotate-12 object-contain opacity-70" />
                <Art name="wood" alt="" class="absolute right-[6%] top-[50%] h-20 w-20 -rotate-6 object-contain opacity-70" />
                <Art name="build-road" alt="" class="absolute left-[20%] top-[8%] h-20 w-20 rotate-45 object-contain opacity-50" />
                <Art name="build-settlement" alt="" class="absolute bottom-[18%] right-[23%] h-20 w-20 -rotate-6 object-contain opacity-50" />
                <Art name="build-city" alt="" class="absolute bottom-[7%] left-[28%] h-24 w-24 rotate-6 object-contain opacity-45" />
                <Art name="build-dev-card" alt="" class="absolute right-[25%] top-[7%] h-24 w-20 -rotate-12 object-contain opacity-45" />
            </div>

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
                <div class="mb-6 flex items-center gap-2 text-white animate-pulse">
                    <div class="w-2 h-2 bg-white rounded-full"></div>
                    "Connecting to server..."
                </div>
            </Show>

            <div class="relative z-10 p-8 sm:p-12 bg-white/95 rounded-[2rem] border-2 border-white/80 shadow-2xl shadow-[#06384a]/30 text-center space-y-8 w-full max-w-2xl overflow-hidden backdrop-blur">
                <div class="absolute inset-x-0 top-0 h-2 bg-gradient-to-r from-[#1689aa] via-[#83dff0] to-white"></div>

                <div class="space-y-2">
                    <p class="text-[#1689aa] font-black uppercase text-[10px] tracking-[0.35em]">"Build the island"</p>
                    <h1 class="text-5xl sm:text-6xl font-black text-[#123c4d] italic tracking-tighter drop-shadow-sm">"COLONIST"</h1>
                    <p class="text-[#5f8490] text-sm">"A quiet table. A crowded island. Your move."</p>
                    <div class="flex items-end justify-center gap-1.5 pt-3 opacity-90">
                        <Art name="wood" alt="" class="h-8 w-8 object-contain" />
                        <Art name="brick" alt="" class="h-8 w-8 object-contain" />
                        <Art name="sheep" alt="" class="h-8 w-8 object-contain" />
                        <Art name="wheat" alt="" class="h-8 w-8 object-contain" />
                        <Art name="ore" alt="" class="h-8 w-8 object-contain" />
                        <Art name="build-settlement" alt="" class="h-10 w-10 object-contain -ml-1" />
                        <Art name="build-city" alt="" class="h-10 w-10 w-10 object-contain -ml-2" />
                    </div>
                </div>

                <div class="text-left space-y-1">
                        <label class="text-[#477482] font-bold uppercase text-[10px] tracking-widest">"Your name"</label>
                    <input
                        class=FIELD
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
                            <h2 class="text-[#477482] font-bold uppercase text-xs tracking-widest">"Active Lobbies"</h2>
                            <button
                                on:click=move |_| set_view_list.set(false)
                                class="text-[#5f8490] hover:text-[#123c4d] text-xs underline transition-colors"
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
                                <div class="py-10 text-[#7b9eaa] italic text-sm">"No games found..."</div>
                            </Show>
                        </div>

                        <button
                            on:click=move |_| state.send(ClientRequest::GetLobbyList)
                            class="text-xs text-[#1689aa]/70 hover:text-[#1689aa] transition-colors flex items-center justify-center gap-1 w-full"
                        >
                            <Icon kind=IconKind::Refresh class="mr-1" /> "Refresh list"
                        </button>
                    </div>
                </Show>
            </div>

            <p class="mt-8 text-white/75 text-[10px] uppercase tracking-[0.3em]">"Powered by Rust & Leptos"</p>
        </div>
    }
}

#[component]
fn ActionButtons(set_view_list: WriteSignal<bool>, state: GameState) -> impl IntoView {
    let (creating, set_creating) = create_signal(false);
    let (table_size, set_table_size) = create_signal(4usize);
    let (game_name, set_game_name) = create_signal(String::new());
    let (selected_colour, set_selected_colour) = create_signal(None::<PlayerColour>);
    let (map_style, set_map_style) = create_signal("classic".to_string());

    let can_create = move || state.ws_sender.get().is_some() && selected_colour.get().is_some();

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
                        class="border-2 border-[#9ed9e8] hover:border-[#1689aa] text-[#477482] hover:text-[#123c4d] py-4 rounded-xl font-bold transition-all disabled:opacity-20"
                        disabled=move || state.ws_sender.get().is_none()
                        on:click=move |_| set_creating.set(true)
                    >
                        "CREATE NEW GAME"
                    </button>
                }
            >
                <div class="space-y-5 text-left">
                    <div class="flex items-center justify-between">
                        <p class="text-[#477482] font-bold uppercase text-[10px] tracking-widest">"Game details"</p>
                    </div>

                    <div class="space-y-2">
                        <label class="text-[#477482] font-bold uppercase text-[10px] tracking-widest">"Game name"</label>
                        <input
                            class=FIELD
                            maxlength=40
                            placeholder="New Colonist Game"
                            prop:value=move || game_name.get()
                            on:input=move |ev| set_game_name.set(event_target_value(&ev))
                        />
                    </div>

                    <div class="space-y-2">
                        <p class="text-[#477482] font-bold uppercase text-[10px] tracking-widest">"Table size"</p>
                        <div class="grid grid-cols-4 sm:grid-cols-7 gap-2">
                            {GameRules::SUPPORTED_PLAYER_COUNTS.into_iter().map(|n| {
                                view! {
                                    <button
                                        class=move || if table_size.get() == n {
                                            "py-2 rounded-lg font-black text-sm bg-[#56c6e1] text-[#06384a] border-2 border-[#1689aa]".to_string()
                                        } else {
                                            "py-2 rounded-lg font-bold text-sm bg-white text-[#477482] border-2 border-[#b8e3ec] hover:border-[#56c6e1]".to_string()
                                        }
                                        on:click=move |_| set_table_size.set(n)
                                    >
                                        {n}
                                    </button>
                                }
                            }).collect_view()}
                        </div>
                        <p class="text-sm font-semibold leading-relaxed text-[#477482]">
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

                    <div class="space-y-2">
                        <label class="text-[#477482] font-bold uppercase text-[10px] tracking-widest">"Map"</label>
                        <select
                            class="w-full bg-white border border-[#9ed9e8] focus:border-[#1689aa] outline-none rounded-xl px-4 py-3 text-[#123c4d]"
                            prop:value=move || map_style.get()
                            on:change=move |ev| set_map_style.set(event_target_value(&ev))
                        >
                            <option value="classic">"Classic island"</option>
                        </select>
                    </div>

                    // A brand new game has the whole palette free.
                    <ColourPicker
                        state=state
                        available=PlayerColour::ALL.to_vec()
                        label="Pick your colour"
                        selected=Signal::derive(move || selected_colour.get())
                        on_pick=Callback::new(move |colour: PlayerColour| {
                            set_selected_colour.set(Some(colour));
                        })
                    />

                    <button
                        class=format!("{} w-full py-3.5 text-base", BTN_PRIMARY)
                        disabled=move || !can_create()
                        on:click=move |_| {
                            if let Some(colour) = selected_colour.get_untracked() {
                                state.send(ClientRequest::CreateGame {
                                    player_count: table_size.get_untracked(),
                                    game_name: game_name.get_untracked(),
                                    seat: state.seat_request(colour),
                                });
                                set_creating.set(false);
                            }
                        }
                    >
                        "CREATE GAME"
                    </button>

                    <button
                        on:click=move |_| set_creating.set(false)
                        class="w-full py-2 text-[#5f8490] hover:text-[#123c4d] text-xs font-bold transition-colors"
                    >
                        "Back"
                    </button>
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
    #[prop(optional)] selected: Option<Signal<Option<PlayerColour>>>,
    on_pick: Callback<PlayerColour>,
) -> impl IntoView {
    let _ = state;

    view! {
        <div class="space-y-3 animate-in zoom-in-95 duration-200">
            <p class="text-[#477482] font-bold uppercase text-[10px] tracking-widest text-left">{label}</p>
            <div class="grid grid-cols-4 gap-3">
                {available.into_iter().map(|colour| {
                    view! {
                        <button
                            class=move || format!(
                                "group flex flex-col items-center gap-1.5 rounded-xl border-2 p-2 transition-all active:scale-95 {}",
                                if selected.is_some_and(|signal| signal.get() == Some(colour)) {
                                    "border-[#1689aa] bg-[#d7f5fa] ring-2 ring-[#56c6e1]/50"
                                } else {
                                    "border-[#b8e3ec] bg-white hover:border-[#56c6e1]"
                                }
                            )
                            title=colour.label()
                            on:click=move |_| on_pick.call(colour)
                        >
                            <span
                                class="w-8 h-8 rounded-full shadow-inner ring-2 ring-black/40"
                                style=format!("background-color: {}", colour.hex())
                            ></span>
                            <span class="text-[9px] uppercase tracking-wider text-[#5f8490] group-hover:text-[#123c4d]">
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

    let LobbyGameInfo { game_id, game_name, players, max_players, available_colours, victory_points_to_win, started } = lobby;
    // A game can begin before its seats are full, so a table with room in it
    // is still closed once play has started - the server refuses the join.
    let is_full = started || players >= max_players || available_colours.is_empty();
    let gid = game_id.clone();

    view! {
        <div class="p-4 bg-slate-950/70 border border-slate-800 rounded-2xl hover:border-orange-500/50 transition-all space-y-3">
            <div class="flex items-center justify-between">
                <div class="text-left">
                    <div class="text-slate-100 font-black text-base">{game_name}</div>
                    <div class="font-mono text-orange-500/80 font-bold text-[10px]">"TABLE " {game_id}</div>
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
