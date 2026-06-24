use anyhow::Result;
use chrono::Utc;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use larder_core::{
    db::{SqlitePool, init_db},
    models::{Difficulty, Recipe, RecipeIngredient, RecipeStep, ShoppingListItem},
    services::{ImportService, MealPlanService, RecipeService, ShoppingListService},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::collections::HashMap;
use std::io;
use uuid::Uuid;

mod app_mode;
mod ui;

use app_mode::AppMode;
use ui::editor::EditorState;
use ui::import::ImportState;
use ui::meal_plan::MealPlanState;
use ui::recipe_detail::RecipeDetailState;
use ui::recipe_list::RecipeListState;
use ui::shopping_list::ShoppingListState;

struct App {
    mode: AppMode,
    should_quit: bool,
    show_help: bool,
    recipe_list: RecipeListState,
    recipe_detail: Option<RecipeDetailState>,
    recipe_editor: Option<EditorState>,
    import: ImportState,
    meal_plan: MealPlanState,
    shopping_list: ShoppingListState,
    recipes: RecipeService,
    importer: ImportService,
    meal_plans: MealPlanService,
    shopping: ShoppingListService,
    status_message: String,
}

impl App {
    fn new(pool: SqlitePool) -> Self {
        let recipes = RecipeService::new(pool.clone());
        let meal_plans = MealPlanService::new(pool.clone());
        let shopping = ShoppingListService::new(pool.clone());
        Self {
            mode: AppMode::RecipeList,
            should_quit: false,
            show_help: false,
            recipe_list: RecipeListState::new(),
            recipe_detail: None,
            recipe_editor: None,
            import: ImportState::new(),
            meal_plan: MealPlanState::new(),
            shopping_list: ShoppingListState::new(),
            recipes,
            importer: ImportService::new(),
            meal_plans,
            shopping,
            status_message: String::new(),
        }
    }

    async fn load_recipes(&mut self) -> Result<()> {
        let user_id = Uuid::nil();
        let all = self.recipes.list_recipes(user_id).await?;
        self.recipe_list.set_recipes(all);
        Ok(())
    }

    async fn load_meal_plan(&mut self) -> Result<()> {
        let user_id = Uuid::nil();
        let meals = self
            .meal_plans
            .get_week(user_id, self.meal_plan.week_start)
            .await?;

        let mut recipe_names = HashMap::new();
        for meal in &meals {
            if let Some(recipe_id) = meal.recipe_id {
                if !recipe_names.contains_key(&recipe_id) {
                    if let Some(recipe) = self.recipes.get_recipe(recipe_id).await? {
                        recipe_names.insert(recipe_id, recipe.name);
                    }
                }
            }
        }

        self.meal_plan.set_meals(meals, recipe_names);
        Ok(())
    }

    async fn load_shopping_list(&mut self) -> Result<()> {
        let user_id = Uuid::nil();
        let items = self.shopping.get_list(user_id).await?;
        self.shopping_list.set_items(items);
        Ok(())
    }

    async fn generate_shopping_from_meal_plan(&mut self) -> Result<()> {
        let user_id = Uuid::nil();
        let count = self
            .shopping
            .generate_from_meal_plan(user_id, self.meal_plan.week_start)
            .await?;
        self.status_message = format!("Added {} items to shopping list", count);
        self.load_shopping_list().await?;
        Ok(())
    }

    async fn assign_meal_recipe(&mut self, recipe_id: Uuid) -> Result<()> {
        let user_id = Uuid::nil();
        self.meal_plans
            .set_recipe(
                user_id,
                self.meal_plan.current_date(),
                self.meal_plan.current_meal_type(),
                recipe_id,
            )
            .await?;
        self.load_meal_plan().await?;
        self.status_message = "Meal assigned".to_string();
        Ok(())
    }

    async fn clear_meal_slot(&mut self) -> Result<()> {
        let user_id = Uuid::nil();
        self.meal_plans
            .clear_slot(
                user_id,
                self.meal_plan.current_date(),
                self.meal_plan.current_meal_type(),
            )
            .await?;
        self.load_meal_plan().await?;
        self.status_message = "Meal cleared".to_string();
        Ok(())
    }

    async fn delete_current_recipe(&mut self) -> Result<()> {
        if let Some(detail) = &self.recipe_detail {
            let id = detail.recipe_id();
            self.recipes.delete_recipe(id).await?;
            self.recipe_detail = None;
            self.mode = AppMode::RecipeList;
            self.load_recipes().await?;
            self.status_message = "Recipe deleted".to_string();
        }
        Ok(())
    }

    async fn save_editor(&mut self) -> Result<()> {
        let Some(editor) = &self.recipe_editor else {
            return Ok(());
        };
        let recipe_id = editor.recipe_id();
        match editor.build_recipe() {
            Ok(recipe) => {
                self.recipes.update_recipe(&recipe).await?;
                self.recipe_editor = None;
                self.load_recipe_detail(recipe_id).await?;
                self.mode = AppMode::RecipeDetail;
                self.status_message = "Recipe saved".to_string();
            }
            Err(msg) => {
                if let Some(editor) = &mut self.recipe_editor {
                    editor.set_status(msg);
                }
            }
        }
        Ok(())
    }

    async fn add_shopping_item(&mut self, text: String) -> Result<()> {
        let item = text.trim();
        if item.is_empty() {
            return Ok(());
        }
        let user_id = Uuid::nil();
        self.shopping
            .add_item(&ShoppingListItem {
                id: Uuid::new_v4(),
                user_id,
                item: item.to_string(),
                quantity: None,
                unit: None,
                category: Some("Other".to_string()),
                checked: false,
                recipe_id: None,
                created_at: Utc::now(),
            })
            .await?;
        self.load_shopping_list().await?;
        self.status_message = format!("Added: {}", item);
        Ok(())
    }

    async fn add_recipe_to_shopping(&mut self, recipe_id: Uuid) -> Result<()> {
        let user_id = Uuid::nil();
        let count = self
            .shopping
            .generate_from_recipes(user_id, &[recipe_id])
            .await?;
        self.status_message = format!("Added {} items to shopping list", count);
        Ok(())
    }

    fn in_text_input(&self) -> bool {
        match self.mode {
            AppMode::Import | AppMode::RecipeEditor => true,
            AppMode::RecipeList => self.recipe_list.search_active(),
            AppMode::ShoppingList => self.shopping_list.adding_item(),
            _ => false,
        }
    }

    fn navigate_to(&mut self, mode: AppMode, rt: &tokio::runtime::Runtime) {
        self.show_help = false;
        match mode {
            AppMode::RecipeList => {
                let _ = rt.block_on(self.load_recipes());
            }
            AppMode::MealPlan => {
                let _ = rt.block_on(self.load_meal_plan());
            }
            AppMode::ShoppingList => {
                let _ = rt.block_on(self.load_shopping_list());
            }
            AppMode::Import => {
                self.import.clear();
            }
            _ => {}
        }
        if mode != AppMode::MealPlanPick {
            self.recipe_list.set_pick_mode(false);
        }
        self.mode = mode;
    }

    fn go_back(&mut self) {
        self.show_help = false;
        self.mode = match self.mode {
            AppMode::RecipeDetail => AppMode::RecipeList,
            AppMode::RecipeEditor => AppMode::RecipeDetail,
            AppMode::MealPlanPick => AppMode::MealPlan,
            _ => AppMode::RecipeList,
        };
    }

    async fn load_recipe_detail(&mut self, id: Uuid) -> Result<()> {
        if let Some(recipe) = self.recipes.get_recipe(id).await? {
            let ingredients = self.recipes.get_ingredients(id).await?;
            let steps = self.recipes.get_steps(id).await?;
            let tags = self.recipes.get_tags(id).await?;
            self.recipe_detail = Some(RecipeDetailState::new(recipe, ingredients, steps, tags));
        }
        Ok(())
    }

    async fn import_recipe_from_url(&mut self, url: &str) -> Result<()> {
        self.import.importing = true;
        let imported = self.importer.import_from_url(url).await?;

        let recipe_id = self.recipes.create_recipe(&imported.recipe).await?;

        for mut ing in imported.ingredients {
            ing.recipe_id = recipe_id;
            self.recipes.add_ingredient(&ing).await?;
        }

        for mut step in imported.steps {
            step.recipe_id = recipe_id;
            self.recipes.add_step(&step).await?;
        }

        self.import.importing = false;
        self.import.status = format!("Imported: {}", imported.recipe.name);
        self.status_message = format!("Imported: {}", imported.recipe.name);
        self.load_recipes().await?;
        Ok(())
    }

    async fn create_sample_recipe(&mut self) -> Result<()> {
        let recipe = Recipe {
            id: Uuid::new_v4(),
            name: "Pasta".to_string(),
            description: Some("Tomato sauce and garlic".to_string()),
            image_url: None,
            servings: 2,
            prep_time_minutes: Some(5),
            cook_time_minutes: Some(15),
            total_time_minutes: Some(20),
            source_url: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            user_id: Uuid::nil(),
            rating: Some(4),
            difficulty: Some(Difficulty::Easy),
        };

        let id = self.recipes.create_recipe(&recipe).await?;

        let ingredients = vec![
            RecipeIngredient {
                id: Uuid::new_v4(),
                recipe_id: id,
                ingredient: "pasta".to_string(),
                quantity: Some(200.into()),
                unit: Some("g".to_string()),
                note: None,
                display: "200 g pasta".to_string(),
                category: Some("Pantry".to_string()),
            },
            RecipeIngredient {
                id: Uuid::new_v4(),
                recipe_id: id,
                ingredient: "tomato sauce".to_string(),
                quantity: Some(1.into()),
                unit: Some("cup".to_string()),
                note: None,
                display: "1 cup tomato sauce".to_string(),
                category: Some("Canned".to_string()),
            },
            RecipeIngredient {
                id: Uuid::new_v4(),
                recipe_id: id,
                ingredient: "garlic".to_string(),
                quantity: Some(2.into()),
                unit: Some("cloves".to_string()),
                note: Some("minced".to_string()),
                display: "2 cloves garlic, minced".to_string(),
                category: Some("Produce".to_string()),
            },
        ];

        for ing in ingredients {
            self.recipes.add_ingredient(&ing).await?;
        }

        let steps = vec![
            RecipeStep {
                id: Uuid::new_v4(),
                recipe_id: id,
                position: 1,
                instruction: "Bring a large pot of salted water to boil. Add pasta and cook according to package directions.".to_string(),
                timer_seconds: Some(600),
            },
            RecipeStep {
                id: Uuid::new_v4(),
                recipe_id: id,
                position: 2,
                instruction: "While pasta cooks, heat olive oil in a pan. Add minced garlic and cook for 1 minute until fragrant.".to_string(),
                timer_seconds: Some(60),
            },
            RecipeStep {
                id: Uuid::new_v4(),
                recipe_id: id,
                position: 3,
                instruction: "Add tomato sauce to the pan. Season with salt and pepper. Simmer for 5 minutes.".to_string(),
                timer_seconds: Some(300),
            },
            RecipeStep {
                id: Uuid::new_v4(),
                recipe_id: id,
                position: 4,
                instruction: "Drain pasta and toss with sauce. Serve with parmesan if desired.".to_string(),
                timer_seconds: None,
            },
        ];

        for step in steps {
            self.recipes.add_step(&step).await?;
        }

        self.status_message = format!("Created recipe: {}", recipe.name);
        self.load_recipes().await?;
        Ok(())
    }

    fn run(&mut self) -> Result<()> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let rt = tokio::runtime::Runtime::new()?;
        let result = self.main_loop(&mut terminal, &rt);

        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        result
    }

    fn main_loop(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        rt: &tokio::runtime::Runtime,
    ) -> Result<()> {
        rt.block_on(self.load_recipes())?;

        loop {
            if self.should_quit {
                return Ok(());
            }

            if let Some(detail) = &mut self.recipe_detail {
                if detail.cooking_mode() {
                    detail.tick();
                }
            }

            terminal.draw(|frame| {
                let (content, nav) = ui::content_and_nav(frame.area());

                if !self.show_help {
                    match self.mode {
                        AppMode::RecipeList | AppMode::MealPlanPick => {
                            ui::recipe_list::render(
                                frame,
                                content,
                                &mut self.recipe_list,
                                &self.status_message,
                            );
                        }
                        AppMode::RecipeDetail => {
                            if let Some(detail) = &self.recipe_detail {
                                ui::recipe_detail::render(frame, content, detail);
                            }
                        }
                        AppMode::RecipeEditor => {
                            if let Some(editor) = &self.recipe_editor {
                                ui::editor::render(frame, content, editor);
                            }
                        }
                        AppMode::Import => ui::import::render(frame, content, &self.import),
                        AppMode::MealPlan => {
                            ui::meal_plan::render(frame, content, &mut self.meal_plan);
                        }
                        AppMode::ShoppingList => {
                            ui::shopping_list::render(frame, content, &mut self.shopping_list);
                        }
                    }
                    ui::status_bar::render(frame, nav, self.mode.nav_tab());
                } else {
                    ui::help::render(frame, frame.area());
                }
            })?;

            if event::poll(std::time::Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        self.handle_key(key.code, key.modifiers, rt);
                    }
                }
            }
        }
    }

    fn handle_key(
        &mut self,
        key: KeyCode,
        _modifiers: KeyModifiers,
        rt: &tokio::runtime::Runtime,
    ) {
        if self.show_help {
            if matches!(key, KeyCode::Esc | KeyCode::Char('?')) {
                self.show_help = false;
            }
            return;
        }

        if key == KeyCode::Char('?') {
            self.show_help = true;
            return;
        }

        if key == KeyCode::Char('q') && !self.in_text_input() {
            self.should_quit = true;
            return;
        }

        if !self.in_text_input() {
            match key {
                KeyCode::Char('1') | KeyCode::Char('r') => {
                    self.navigate_to(AppMode::RecipeList, rt);
                    return;
                }
                KeyCode::Char('2') | KeyCode::Char('i') => {
                    self.navigate_to(AppMode::Import, rt);
                    return;
                }
                KeyCode::Char('3') | KeyCode::Char('m') => {
                    self.navigate_to(AppMode::MealPlan, rt);
                    return;
                }
                KeyCode::Char('4') | KeyCode::Char('s') => {
                    self.navigate_to(AppMode::ShoppingList, rt);
                    return;
                }
                KeyCode::Char('b') => {
                    self.go_back();
                    return;
                }
                _ => {}
            }
        }

        match self.mode {
            AppMode::Import => match key {
                KeyCode::Enter if !self.import.url.is_empty() && !self.import.importing => {
                    let url = self.import.url.clone();
                    if let Err(e) = rt.block_on(self.import_recipe_from_url(&url)) {
                        self.import.importing = false;
                        self.import.status = format!("Error: {}", e);
                    }
                }
                KeyCode::Esc if !self.import.importing => {
                    self.navigate_to(AppMode::RecipeList, rt);
                }
                KeyCode::Char(c) if !self.import.importing => {
                    self.import.push_char(c);
                }
                KeyCode::Backspace if !self.import.importing => {
                    self.import.backspace();
                }
                KeyCode::Delete if !self.import.importing => {
                    self.import.delete();
                }
                KeyCode::Left if !self.import.importing => {
                    self.import.move_left();
                }
                KeyCode::Right if !self.import.importing => {
                    self.import.move_right();
                }
                _ => {}
            },
            AppMode::RecipeList | AppMode::MealPlanPick => match key {
                KeyCode::Char('/') if self.mode == AppMode::RecipeList => {
                    self.recipe_list.toggle_search()
                }
                KeyCode::Char(c) if self.recipe_list.search_active() => {
                    self.recipe_list.push_search(c);
                }
                KeyCode::Backspace if self.recipe_list.search_active() => {
                    self.recipe_list.pop_search();
                }
                KeyCode::Esc if self.recipe_list.search_active() => {
                    self.recipe_list.clear_search();
                }
                KeyCode::Esc if self.mode == AppMode::MealPlanPick => {
                    self.go_back();
                }
                KeyCode::Down | KeyCode::Char('j') => self.recipe_list.select_next(),
                KeyCode::Up | KeyCode::Char('k') => self.recipe_list.select_previous(),
                KeyCode::Enter => {
                    if let Some(id) = self.recipe_list.selected_id() {
                        if self.mode == AppMode::MealPlanPick {
                            if let Err(e) = rt.block_on(self.assign_meal_recipe(id)) {
                                self.status_message = format!("Error: {}", e);
                            }
                            self.recipe_list.set_pick_mode(false);
                            self.mode = AppMode::MealPlan;
                        } else if let Err(e) = rt.block_on(self.load_recipe_detail(id)) {
                            self.status_message = format!("Error: {}", e);
                        } else {
                            self.mode = AppMode::RecipeDetail;
                        }
                    }
                }
                KeyCode::Char('n') if self.mode == AppMode::RecipeList => {
                    if let Err(e) = rt.block_on(self.create_sample_recipe()) {
                        self.status_message = format!("Error: {}", e);
                    }
                }
                _ => {}
            },
            AppMode::RecipeEditor => match key {
                KeyCode::Esc => {
                    self.recipe_editor = None;
                    self.mode = AppMode::RecipeDetail;
                }
                KeyCode::Tab | KeyCode::Down => {
                    if let Some(editor) = &mut self.recipe_editor {
                        editor.next_field();
                    }
                }
                KeyCode::BackTab | KeyCode::Up => {
                    if let Some(editor) = &mut self.recipe_editor {
                        editor.prev_field();
                    }
                }
                KeyCode::Enter => {
                    if let Err(e) = rt.block_on(self.save_editor()) {
                        self.status_message = format!("Error: {}", e);
                    }
                }
                KeyCode::Backspace => {
                    if let Some(editor) = &mut self.recipe_editor {
                        editor.backspace();
                    }
                }
                KeyCode::Char(c) => {
                    if let Some(editor) = &mut self.recipe_editor {
                        editor.push_char(c);
                    }
                }
                _ => {}
            },
            AppMode::RecipeDetail => {
                let in_cooking = self
                    .recipe_detail
                    .as_ref()
                    .map(|d| d.cooking_mode())
                    .unwrap_or(false);
                if in_cooking {
                    match key {
                        KeyCode::Esc => {
                            if let Some(d) = &mut self.recipe_detail {
                                d.toggle_cooking_mode();
                            }
                        }
                        KeyCode::Down | KeyCode::Char('j') | KeyCode::Right => {
                            if let Some(d) = &mut self.recipe_detail {
                                d.next_step();
                            }
                        }
                        KeyCode::Up | KeyCode::Char('k') | KeyCode::Left => {
                            if let Some(d) = &mut self.recipe_detail {
                                d.prev_step();
                            }
                        }
                        KeyCode::Char(' ') => {
                            if let Some(d) = &mut self.recipe_detail {
                                d.toggle_timer();
                            }
                        }
                        _ => {}
                    }
                } else {
                    match key {
                        KeyCode::Esc | KeyCode::Char('b') => self.mode = AppMode::RecipeList,
                        KeyCode::Down | KeyCode::Char('j') => {
                            if let Some(d) = &mut self.recipe_detail {
                                d.scroll_down();
                            }
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            if let Some(d) = &mut self.recipe_detail {
                                d.scroll_up();
                            }
                        }
                        KeyCode::Char('c') => {
                            if let Some(d) = &mut self.recipe_detail {
                                d.toggle_cooking_mode();
                            }
                        }
                        KeyCode::Char('e') => {
                            if let Some(detail) = &self.recipe_detail {
                                self.recipe_editor =
                                    Some(EditorState::new(detail.recipe().clone()));
                                self.mode = AppMode::RecipeEditor;
                            }
                        }
                        KeyCode::Char('d') => {
                            if let Err(e) = rt.block_on(self.delete_current_recipe()) {
                                self.status_message = format!("Error: {}", e);
                            }
                        }
                        KeyCode::Char('+') | KeyCode::Char('=') => {
                            if let Some(d) = &mut self.recipe_detail {
                                d.scale_up();
                            }
                        }
                        KeyCode::Char('-') => {
                            if let Some(d) = &mut self.recipe_detail {
                                d.scale_down();
                            }
                        }
                        KeyCode::Char('g') => {
                            if let Some(detail) = &self.recipe_detail {
                                let id = detail.recipe_id();
                                if let Err(e) = rt.block_on(self.add_recipe_to_shopping(id)) {
                                    self.status_message = format!("Error: {}", e);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            AppMode::MealPlan => match key {
                KeyCode::Right | KeyCode::Char('l') => {
                    self.meal_plan.navigate_next_day();
                    if let Err(e) = rt.block_on(self.load_meal_plan()) {
                        self.status_message = format!("Error: {}", e);
                    }
                }
                KeyCode::Left | KeyCode::Char('h') => {
                    self.meal_plan.navigate_prev_day();
                    if let Err(e) = rt.block_on(self.load_meal_plan()) {
                        self.status_message = format!("Error: {}", e);
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => self.meal_plan.navigate_next_slot(),
                KeyCode::Up | KeyCode::Char('k') => self.meal_plan.navigate_prev_slot(),
                KeyCode::Char(']') => {
                    self.meal_plan.week_forward();
                    if let Err(e) = rt.block_on(self.load_meal_plan()) {
                        self.status_message = format!("Error: {}", e);
                    }
                }
                KeyCode::Char('[') => {
                    self.meal_plan.week_backward();
                    if let Err(e) = rt.block_on(self.load_meal_plan()) {
                        self.status_message = format!("Error: {}", e);
                    }
                }
                KeyCode::Char('a') => {
                    self.recipe_list.set_pick_mode(true);
                    self.mode = AppMode::MealPlanPick;
                }
                KeyCode::Char('d') => {
                    if let Err(e) = rt.block_on(self.clear_meal_slot()) {
                        self.status_message = format!("Error: {}", e);
                    }
                }
                KeyCode::Char('g') => {
                    if let Err(e) = rt.block_on(self.generate_shopping_from_meal_plan()) {
                        self.status_message = format!("Error: {}", e);
                    }
                }
                _ => {}
            },
            AppMode::ShoppingList => {
                if self.shopping_list.adding_item() {
                    match key {
                        KeyCode::Esc => self.shopping_list.cancel_add_item(),
                        KeyCode::Enter => {
                            let text = self.shopping_list.take_new_item();
                            if let Err(e) = rt.block_on(self.add_shopping_item(text)) {
                                self.status_message = format!("Error: {}", e);
                            }
                        }
                        KeyCode::Backspace => self.shopping_list.pop_char(),
                        KeyCode::Char(c) => self.shopping_list.push_char(c),
                        _ => {}
                    }
                } else {
                    match key {
                        KeyCode::Down | KeyCode::Char('j') => self.shopping_list.select_next(),
                        KeyCode::Up | KeyCode::Char('k') => self.shopping_list.select_previous(),
                        KeyCode::Char('c') | KeyCode::Char(' ') => {
                            if let Some(id) = self.shopping_list.selected_item_id() {
                                if let Err(e) = rt.block_on(self.shopping.toggle_checked(id)) {
                                    self.status_message = format!("Error: {}", e);
                                } else if let Err(e) = rt.block_on(self.load_shopping_list()) {
                                    self.status_message = format!("Error: {}", e);
                                }
                            }
                        }
                        KeyCode::Char('a') => self.shopping_list.start_add_item(),
                        KeyCode::Char('g') => {
                            if let Err(e) = rt.block_on(self.generate_shopping_from_meal_plan()) {
                                self.status_message = format!("Error: {}", e);
                            } else if let Err(e) = rt.block_on(self.load_shopping_list()) {
                                self.status_message = format!("Error: {}", e);
                            }
                        }
                        KeyCode::Char('x') => {
                            let user_id = Uuid::nil();
                            if let Err(e) = rt.block_on(self.shopping.clear_checked(user_id)) {
                                self.status_message = format!("Error: {}", e);
                            } else {
                                self.status_message = "Cleared checked items".to_string();
                                if let Err(e) = rt.block_on(self.load_shopping_list()) {
                                    self.status_message = format!("Error: {}", e);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

fn main() -> anyhow::Result<()> {
    color_eyre::install().unwrap();

    let rt = tokio::runtime::Runtime::new()?;
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:larder.db".to_string());
    let pool = rt.block_on(init_db(&database_url))?;

    let mut app = App::new(pool);
    app.run()
}
