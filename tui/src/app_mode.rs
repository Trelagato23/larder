#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    RecipeList,
    RecipeDetail,
    RecipeEditor,
    Import,
    MealPlan,
    MealPlanPick,
    ShoppingList,
}

impl AppMode {
    /// Which top nav tab is highlighted (1-4).
    pub fn nav_tab(self) -> u8 {
        match self {
            Self::RecipeList | Self::RecipeDetail | Self::RecipeEditor => 1,
            Self::Import => 2,
            Self::MealPlan | Self::MealPlanPick => 3,
            Self::ShoppingList => 4,
        }
    }
}
