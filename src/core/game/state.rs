//! Game state types — characters, map tiles, world events, economy data.

use serde::{Deserialize, Serialize};

/// Skill level and XP information.
///
/// Tracks current level, current XP, and max XP for level-up calculation.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SkillInfo {
    /// Current skill level (0-N).
    pub level: u32,
    /// Current experience points toward next level.
    pub xp: u32,
    /// Experience points needed to reach next level.
    pub max_xp: u32,
}

impl SkillInfo {
    /// Calculate XP progress ratio (0.0 to 1.0) for progress bars.
    pub fn xp_ratio(&self) -> f64 {
        if self.max_xp == 0 {
            0.0
        } else {
            self.xp as f64 / self.max_xp as f64
        }
    }
}

/// A single inventory slot with item and quantity.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct InventorySlot {
    /// Slot index in the inventory.
    pub slot: u32,
    /// Item code (e.g., "iron_ore", "leather_armor").
    pub code: String,
    /// Quantity of this item in the slot.
    pub quantity: u32,
}

/// An active effect on a character (e.g., poison, blessing, curse).
///
/// Effects come from the API's `effects` array and may represent status conditions
/// with remaining stacks or duration ticks.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CharacterEffect {
    /// Effect display name.
    pub name: String,
    /// Effect code identifier.
    pub code: String,
    /// Remaining stacks or duration ticks (varies by API version).
    pub value: i32,
}

/// Complete character state and statistics.
///
/// Initially populated from `GET /my/characters` at startup and incrementally
/// updated by `account_log` and `online_characters` WebSocket notifications.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CharacterState {
    // ── Identity ──────────────────────────────────────────────────────────
    /// Character name.
    pub name: String,
    /// Account name (empty if not set in account_log).
    pub account: String,
    /// Skin/appearance code (e.g., "human_f", "elf_m").
    pub skin: String,

    // ── Position ──────────────────────────────────────────────────────────
    pub x: i32,
    pub y: i32,
    pub map_id: i32,

    // ── Combat / level ────────────────────────────────────────────────────
    pub level: u32,
    pub xp: u32,
    pub max_xp: u32,
    pub hp: u32,
    pub max_hp: u32,
    pub gold: u32,
    pub speed: i32,

    // ── Attributes ────────────────────────────────────────────────────────
    pub haste: i32,
    pub critical_strike: i32,
    pub wisdom: i32,
    pub prospecting: i32,
    pub initiative: i32,
    pub threat: i32,

    // ── Elemental attack ──────────────────────────────────────────────────
    pub attack_fire: i32,
    pub attack_earth: i32,
    pub attack_water: i32,
    pub attack_air: i32,

    // ── Damage modifiers (%) ──────────────────────────────────────────────
    pub dmg: i32,
    pub dmg_fire: i32,
    pub dmg_earth: i32,
    pub dmg_water: i32,
    pub dmg_air: i32,

    // ── Resistances (%) ───────────────────────────────────────────────────
    pub res_fire: i32,
    pub res_earth: i32,
    pub res_water: i32,
    pub res_air: i32,

    // ── Skills ────────────────────────────────────────────────────────────
    pub mining: SkillInfo,
    pub woodcutting: SkillInfo,
    pub fishing: SkillInfo,
    pub weaponcrafting: SkillInfo,
    pub gearcrafting: SkillInfo,
    pub jewelrycrafting: SkillInfo,
    pub cooking: SkillInfo,
    pub alchemy: SkillInfo,

    // ── Task ──────────────────────────────────────────────────────────────
    pub task: String,
    pub task_type: String,
    pub task_progress: u32,
    pub task_total: u32,

    // ── Equipment slots ───────────────────────────────────────────────────
    pub weapon_slot: String,
    pub rune_slot: String,
    pub shield_slot: String,
    pub helmet_slot: String,
    pub body_armor_slot: String,
    pub leg_armor_slot: String,
    pub boots_slot: String,
    pub ring1_slot: String,
    pub ring2_slot: String,
    pub amulet_slot: String,
    pub artifact1_slot: String,
    pub artifact2_slot: String,
    pub artifact3_slot: String,
    pub utility1_slot: String,
    pub utility1_slot_quantity: u32,
    pub utility2_slot: String,
    pub utility2_slot_quantity: u32,
    pub bag_slot: String,

    // ── Inventory ─────────────────────────────────────────────────────────
    pub inventory: Vec<InventorySlot>,
    pub inventory_max_items: u32,

    // ── Effects ───────────────────────────────────────────────────────────
    pub effects: Vec<CharacterEffect>,

    // ── Cooldown ──────────────────────────────────────────────────────────
    /// Remaining cooldown seconds as last reported by REST/WS.
    pub cooldown_secs: i32,
    /// ISO-8601 expiration timestamp from the REST API.
    pub cooldown_expiration: String,
    pub created_at: String,

    // ── Last WS action ────────────────────────────────────────────────────
    pub last_action: String,
    pub last_description: String,
}

impl Default for CharacterState {
    fn default() -> Self {
        Self {
            name: String::new(),
            account: String::new(),
            skin: String::new(),
            x: 0,
            y: 0,
            map_id: 1,
            level: 0,
            xp: 0,
            max_xp: 0,
            hp: 0,
            max_hp: 0,
            gold: 0,
            speed: 0,
            haste: 0,
            critical_strike: 0,
            wisdom: 0,
            prospecting: 0,
            initiative: 0,
            threat: 0,
            attack_fire: 0,
            attack_earth: 0,
            attack_water: 0,
            attack_air: 0,
            dmg: 0,
            dmg_fire: 0,
            dmg_earth: 0,
            dmg_water: 0,
            dmg_air: 0,
            res_fire: 0,
            res_earth: 0,
            res_water: 0,
            res_air: 0,
            mining: SkillInfo::default(),
            woodcutting: SkillInfo::default(),
            fishing: SkillInfo::default(),
            weaponcrafting: SkillInfo::default(),
            gearcrafting: SkillInfo::default(),
            jewelrycrafting: SkillInfo::default(),
            cooking: SkillInfo::default(),
            alchemy: SkillInfo::default(),
            task: String::new(),
            task_type: String::new(),
            task_progress: 0,
            task_total: 0,
            weapon_slot: String::new(),
            rune_slot: String::new(),
            shield_slot: String::new(),
            helmet_slot: String::new(),
            body_armor_slot: String::new(),
            leg_armor_slot: String::new(),
            boots_slot: String::new(),
            ring1_slot: String::new(),
            ring2_slot: String::new(),
            amulet_slot: String::new(),
            artifact1_slot: String::new(),
            artifact2_slot: String::new(),
            artifact3_slot: String::new(),
            utility1_slot: String::new(),
            utility1_slot_quantity: 0,
            utility2_slot: String::new(),
            utility2_slot_quantity: 0,
            bag_slot: String::new(),
            inventory: Vec::new(),
            inventory_max_items: 0,
            effects: Vec::new(),
            cooldown_secs: 0,
            cooldown_expiration: String::new(),
            created_at: String::new(),
            last_action: "idle".to_string(),
            last_description: String::new(),
        }
    }
}

impl CharacterState {
    pub fn hp_ratio(&self) -> f64 {
        if self.max_hp == 0 {
            1.0
        } else {
            self.hp as f64 / self.max_hp as f64
        }
    }

    pub fn xp_ratio(&self) -> f64 {
        if self.max_xp == 0 {
            0.0
        } else {
            self.xp as f64 / self.max_xp as f64
        }
    }

    /// Non-empty inventory slots, sorted by slot index.
    pub fn items(&self) -> impl Iterator<Item = &InventorySlot> {
        self.inventory
            .iter()
            .filter(|s| !s.code.is_empty() && s.quantity > 0)
    }

    /// Returns true if the character has any non-zero elemental attack or resistance.
    pub fn has_combat_stats(&self) -> bool {
        self.attack_fire != 0
            || self.attack_earth != 0
            || self.attack_water != 0
            || self.attack_air != 0
            || self.res_fire != 0
            || self.res_earth != 0
            || self.res_water != 0
            || self.res_air != 0
            || self.dmg != 0
            || self.haste != 0
            || self.critical_strike != 0
            || self.prospecting != 0
    }

    /// Current task display string, e.g. "kill chicken (3/5)".
    pub fn task_display(&self) -> Option<String> {
        if self.task.is_empty() {
            return None;
        }
        if self.task_total > 0 {
            Some(format!("{} ({}/{})", self.task, self.task_progress, self.task_total))
        } else {
            Some(self.task.clone())
        }
    }

    /// Display label for the current activity.
    pub fn activity_label(&self) -> &str {
        match self.last_action.as_str() {
            "fight" | "multi_fight" => "Fighting",
            "gathering" => "Gathering",
            "crafting" => "Crafting",
            "movement" => "Moving",
            "rest" => "Resting",
            "task_completed" => "Task Done",
            "new_task" => "New Task",
            "task_exchange" | "task_cancelled" => "Task",
            "recycling" => "Recycling",
            "buy_ge" | "sell_ge" | "create_buy_order_ge" | "fill_buy_order_ge" => "Trading",
            "deposit_item" | "withdraw_item" | "deposit_gold" | "withdraw_gold" => "Banking",
            "equip" | "unequip" => "Equipping",
            _ => "Idle",
        }
    }

    /// Accent colour for the activity.
    pub fn activity_color(&self) -> ratatui::style::Color {
        use ratatui::style::Color;
        match self.last_action.as_str() {
            "fight" | "multi_fight" => Color::Red,
            "gathering" => Color::Green,
            "crafting" => Color::Yellow,
            "movement" => Color::Cyan,
            "rest" => Color::Blue,
            "task_completed" | "new_task" | "task_exchange" | "task_cancelled" => Color::Magenta,
            "recycling" => Color::LightYellow,
            "buy_ge" | "sell_ge" | "create_buy_order_ge" | "fill_buy_order_ge" => Color::LightCyan,
            "deposit_item" | "withdraw_item" | "deposit_gold" | "withdraw_gold" => Color::Gray,
            _ => Color::DarkGray,
        }
    }

    /// Apply partial data from the `character` node inside an account_log payload.
    /// Handles the fields that commonly change during gameplay.
    pub fn apply_char_node(&mut self, node: &serde_json::Value) {
        macro_rules! set_u32 {
            ($field:ident, $key:expr) => {
                if let Some(v) = node.get($key).and_then(|v| v.as_u64()) {
                    self.$field = v as u32;
                }
            };
        }
        macro_rules! set_i32 {
            ($field:ident, $key:expr) => {
                if let Some(v) = node.get($key).and_then(|v| v.as_i64()) {
                    self.$field = v as i32;
                }
            };
        }
        macro_rules! set_str {
            ($field:ident, $key:expr) => {
                if let Some(s) = node.get($key).and_then(|v| v.as_str()) {
                    self.$field = s.to_string();
                }
            };
        }

        // Core stats
        set_u32!(hp, "hp");
        set_u32!(max_hp, "max_hp");
        set_u32!(xp, "xp");
        set_u32!(max_xp, "max_xp");
        set_u32!(level, "level");
        set_u32!(gold, "gold");
        set_i32!(x, "x");
        set_i32!(y, "y");
        set_i32!(map_id, "map_id");
        set_i32!(speed, "speed");

        if self.skin.is_empty() {
            set_str!(skin, "skin");
        }

        // Task
        set_str!(task, "task");
        set_str!(task_type, "task_type");
        set_u32!(task_progress, "task_progress");
        set_u32!(task_total, "task_total");

        // Attributes
        set_i32!(haste, "haste");
        set_i32!(critical_strike, "critical_strike");
        set_i32!(wisdom, "wisdom");
        set_i32!(prospecting, "prospecting");
        set_i32!(initiative, "initiative");
        set_i32!(threat, "threat");

        // Elemental attack
        set_i32!(attack_fire, "attack_fire");
        set_i32!(attack_earth, "attack_earth");
        set_i32!(attack_water, "attack_water");
        set_i32!(attack_air, "attack_air");

        // Damage modifiers
        set_i32!(dmg, "dmg");
        set_i32!(dmg_fire, "dmg_fire");
        set_i32!(dmg_earth, "dmg_earth");
        set_i32!(dmg_water, "dmg_water");
        set_i32!(dmg_air, "dmg_air");

        // Resistances
        set_i32!(res_fire, "res_fire");
        set_i32!(res_earth, "res_earth");
        set_i32!(res_water, "res_water");
        set_i32!(res_air, "res_air");

        // Cooldown
        set_i32!(cooldown_secs, "cooldown");
        set_str!(cooldown_expiration, "cooldown_expiration");
    }

    /// Apply the full CharacterSchema from the REST API response (or a complete WS payload).
    pub fn apply_full_schema(&mut self, node: &serde_json::Value) {
        // Apply all shared fields first
        self.apply_char_node(node);

        macro_rules! set_u32 {
            ($field:ident, $key:expr) => {
                if let Some(v) = node.get($key).and_then(|v| v.as_u64()) {
                    self.$field = v as u32;
                }
            };
        }
        macro_rules! set_str {
            ($field:ident, $key:expr) => {
                if let Some(s) = node.get($key).and_then(|v| v.as_str()) {
                    self.$field = s.to_string();
                }
            };
        }
        macro_rules! skill {
            ($field:ident, $prefix:expr) => {
                self.$field = SkillInfo {
                    level: node
                        .get(concat!($prefix, "_level"))
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32,
                    xp: node
                        .get(concat!($prefix, "_xp"))
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32,
                    max_xp: node
                        .get(concat!($prefix, "_max_xp"))
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32,
                };
            };
        }

        // All 8 skills
        skill!(mining, "mining");
        skill!(woodcutting, "woodcutting");
        skill!(fishing, "fishing");
        skill!(weaponcrafting, "weaponcrafting");
        skill!(gearcrafting, "gearcrafting");
        skill!(jewelrycrafting, "jewelrycrafting");
        skill!(cooking, "cooking");
        skill!(alchemy, "alchemy");

        // All equipment slots
        set_str!(weapon_slot, "weapon_slot");
        set_str!(rune_slot, "rune_slot");
        set_str!(shield_slot, "shield_slot");
        set_str!(helmet_slot, "helmet_slot");
        set_str!(body_armor_slot, "body_armor_slot");
        set_str!(leg_armor_slot, "leg_armor_slot");
        set_str!(boots_slot, "boots_slot");
        set_str!(ring1_slot, "ring1_slot");
        set_str!(ring2_slot, "ring2_slot");
        set_str!(amulet_slot, "amulet_slot");
        set_str!(artifact1_slot, "artifact1_slot");
        set_str!(artifact2_slot, "artifact2_slot");
        set_str!(artifact3_slot, "artifact3_slot");
        set_str!(utility1_slot, "utility1_slot");
        set_u32!(utility1_slot_quantity, "utility1_slot_quantity");
        set_str!(utility2_slot, "utility2_slot");
        set_u32!(utility2_slot_quantity, "utility2_slot_quantity");
        set_str!(bag_slot, "bag_slot");

        // Inventory
        set_u32!(inventory_max_items, "inventory_max_items");
        if let Some(arr) = node
            .get("inventory")
            .and_then(|v| v.as_array())
        {
            self.inventory = arr
                .iter()
                .filter_map(|slot| {
                    let code = slot
                        .get("code")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let quantity = slot
                        .get("quantity")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32;
                    let s = slot
                        .get("slot")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32;
                    if code.is_empty() || quantity == 0 {
                        None
                    } else {
                        Some(InventorySlot {
                            slot: s,
                            code,
                            quantity,
                        })
                    }
                })
                .collect();
        }

        // Effects
        if let Some(arr) = node
            .get("effects")
            .and_then(|v| v.as_array())
        {
            self.effects = arr
                .iter()
                .filter_map(|e| {
                    // Effect objects may have `name`, `code`, `value` depending on API version
                    let name = e
                        .get("name")
                        .or_else(|| e.get("code"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    if name.is_empty() {
                        return None;
                    }
                    let code = e
                        .get("code")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let value = e
                        .get("value")
                        .or_else(|| e.get("stacks"))
                        .and_then(|v| v.as_i64())
                        .unwrap_or(0) as i32;
                    Some(CharacterEffect {
                        name,
                        code,
                        value,
                    })
                })
                .collect();
        }
    }
}

/// A parsed account_log websocket notification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccountLogEntry {
    pub character: String,
    pub account: String,
    pub log_type: String,
    pub description: String,
    pub cooldown: i32,
    pub content: serde_json::Value,
}

impl AccountLogEntry {
    /// Find the character data node inside the content payload.
    pub fn character_node(&self) -> Option<&serde_json::Value> {
        if let Some(node) = self
            .content
            .get("character")
            .filter(|v| v.is_object())
        {
            return Some(node);
        }
        if let Some(arr) = self
            .content
            .get("characters")
            .and_then(|v| v.as_array())
        {
            return arr.iter().find(|c| {
                c.get("name")
                    .and_then(|n| n.as_str())
                    .map(|n| n == self.character)
                    .unwrap_or(false)
                    || c.get("character_name")
                        .and_then(|n| n.as_str())
                        .map(|n| n == self.character)
                        .unwrap_or(false)
            });
        }
        None
    }

    pub fn cooldown_remaining_secs(&self) -> f64 {
        self.content
            .get("cooldown")
            .and_then(|c| c.get("remaining_seconds"))
            .and_then(|v| v.as_f64())
            .unwrap_or(self.cooldown as f64)
    }

    pub fn cooldown_total_secs(&self) -> f64 {
        self.content
            .get("cooldown")
            .and_then(|c| c.get("total_seconds"))
            .and_then(|v| v.as_f64())
            .unwrap_or(self.cooldown as f64)
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Shared types for the central GameState
// ──────────────────────────────────────────────────────────────────────────────

/// Single map tile fetched from `GET /maps`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapTile {
    pub x: i32,
    pub y: i32,
    /// Unique tile ID from the API (used to identify a character's current layer).
    pub map_id: i32,
    /// Layer this tile belongs to: `"overworld"`, `"underground"`, `"interior"`, …
    pub layer: String,
    /// Human-readable tile name (e.g. "Forest", "Mine Level 3").
    pub name: String,
    /// Visual skin identifier (maps to `https://artifactsmmo.com/images/maps/{skin}.png`).
    pub skin: String,
    /// Content type: "monster", "resource", "bank", "workshop", "tasks_master",
    /// "grand_exchange", or empty string for empty tiles.
    pub content_type: String,
    /// Content code (monster/resource/workshop code).
    pub content_code: String,
}

/// Which top-level panel currently holds keyboard focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FocusedPanel {
    #[default]
    CharGrid,
    Sidebar,
    LogPanel,
}

impl FocusedPanel {
    pub fn next(self) -> Self {
        match self {
            Self::CharGrid => Self::Sidebar,
            Self::Sidebar  => Self::LogPanel,
            Self::LogPanel => Self::CharGrid,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::CharGrid => Self::LogPanel,
            Self::Sidebar  => Self::CharGrid,
            Self::LogPanel => Self::Sidebar,
        }
    }
}

/// WebSocket connection status.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum WsStatus {
    #[default]
    Connecting,
    Connected,
    Disconnected(String),
}

/// A single entry in the sidebar GE feed.
#[derive(Debug, Clone)]
pub enum GEFeedEntry {
    Order(GEOrder),
    Transaction(GETransaction),
}

/// A single entry in the global footer log.
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub tag: &'static str,
    pub tag_color: ratatui::style::Color,
    pub character: String,
    pub message: String,
}

/// Central game state, shared between the app loop (writer) and components (readers)
/// via `Arc<RwLock<GameState>>`.
#[derive(Debug)]
pub struct GameState {
    // Characters
    pub characters: Vec<CharacterState>,
    pub cooldown_expires: std::collections::HashMap<String, std::time::Instant>,
    pub cooldown_totals: std::collections::HashMap<String, f64>,
    pub action_history: std::collections::HashMap<String, std::collections::VecDeque<ActionRecord>>,

    // World
    pub world_events: Vec<WorldEvent>,
    pub ge_feed: std::collections::VecDeque<GEFeedEntry>,
    pub ws_status: WsStatus,

    // Footer log
    pub log_entries: std::collections::VecDeque<LogEntry>,

    // Map
    /// Keyed by `(x, y, layer)` — each coordinate has one tile per layer name.
    pub map_tiles: std::collections::HashMap<(i32, i32, String), MapTile>,
    /// Maps a tile's unique `map_id` to its layer string.  Used to look up
    /// which layer the character is currently on from `CharacterState::map_id`.
    pub map_id_to_layer: std::collections::HashMap<i32, String>,

    // Animation
    /// Toggled on every Tick so cards can show a heartbeat icon.
    pub heartbeat: bool,

    // Economy
    /// `(Instant, total_gold_across_all_chars)` snapshots for gold/hr calc.
    pub gold_snapshots: std::collections::VecDeque<(std::time::Instant, u64)>,

    // Demand
    /// Swarm demand from the local bot API.
    pub swarm_demand: Vec<(String, u32)>,

    // UI selection
    /// Index of the currently selected character in `characters`.
    pub selected_character: usize,
    /// Which top-level panel holds keyboard focus.
    pub focused_panel: FocusedPanel,
}

impl Default for GameState {
    fn default() -> Self {
        Self {
            characters: Vec::new(),
            cooldown_expires: std::collections::HashMap::new(),
            cooldown_totals: std::collections::HashMap::new(),
            action_history: std::collections::HashMap::new(),
            world_events: Vec::new(),
            ge_feed: std::collections::VecDeque::new(),
            ws_status: WsStatus::Connecting,
            log_entries: std::collections::VecDeque::new(),
            map_tiles: std::collections::HashMap::new(),
            map_id_to_layer: std::collections::HashMap::new(),
            heartbeat: false,
            gold_snapshots: std::collections::VecDeque::new(),
            swarm_demand: Vec::new(),
            selected_character: 0,
            focused_panel: FocusedPanel::default(),
        }
    }
}

impl GameState {
    /// Returns estimated gold earned per hour across all characters,
    /// based on snapshots taken over the last 10 minutes.
    pub fn gold_per_hour(&self) -> Option<f64> {
        let snaps: Vec<_> = self.gold_snapshots.iter().collect();
        if snaps.len() < 2 {
            return None;
        }
        let (t0, g0) = snaps.first().unwrap();
        let (t1, g1) = snaps.last().unwrap();
        let elapsed_hours = t1.duration_since(*t0).as_secs_f64() / 3600.0;
        if elapsed_hours < 0.001 {
            return None;
        }
        Some((*g1 as f64 - *g0 as f64) / elapsed_hours)
    }

    /// Record current total gold for gold/hr calculation (keep last 20 snapshots).
    pub fn snapshot_gold(&mut self) {
        let total: u64 = self
            .characters
            .iter()
            .map(|c| c.gold as u64)
            .sum();
        let now = std::time::Instant::now();
        if self.gold_snapshots.len() >= 20 {
            self.gold_snapshots.pop_front();
        }
        self.gold_snapshots
            .push_back((now, total));
    }
}

/// A compact record for the per-character action history list.
#[derive(Debug, Clone)]
pub struct ActionRecord {
    pub log_type: String,
    pub description: String,
}

/// An active world event (from event_spawn).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorldEvent {
    pub name: String,
    pub code: String,
    pub expiration: String,
}

/// A Grand Exchange order notification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GEOrder {
    pub order_type: String,
    pub code: String,
    pub quantity: u32,
    pub price: u32,
    pub account: Option<String>,
}

/// A completed Grand Exchange transaction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GETransaction {
    pub order_type: String,
    pub code: String,
    pub quantity: u32,
    pub total_price: u32,
}
