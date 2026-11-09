# Organizing recipes

Collections imported from other apps are often uncategorised and spelled inconsistently.
Larder can sort recipes into categories and tidy their ingredient lines for you, either as
recipes come in or in bulk afterwards.

## Larder's categories

Larder suggests one of twelve categories, listed here in the order they appear as chapters in
the [printable cookbook](cookbook.md#chapters):

1. Breakfast
2. Appetizers & Snacks
3. Soups & Stews
4. Salads
5. Main Dishes
6. Side Dishes
7. Breads
8. Desserts
9. Drinks
10. Sauces & Condiments
11. Seasonings & Spice Mixes
12. Household & DIY

They are ordinary categories: you can rename them, remove them or add your own in the
[editor](editing.md). Your own categories work the same way everywhere.

## How a category is chosen

The suggestion comes from keywords, mainly in the recipe **name**:

1. **The dish is named last.** English dish names put the dish at the end ("Chicken Noodle
   **Soup**", "Pork **Salad**"), so the category whose keyword ends latest in the name wins.
   Longer keywords break ties ("soup mix" beats "soup"), then the category order above.
2. **Only the dish counts.** Anything in parentheses or braces and anything after "with" or
   "without" is ignored: "Pepperoni Pizza with Peppers and Olives" is a main dish, and
   "Apple Pie without Refined Sugar" a dessert.
3. **A main with a side is a main.** If the name points to a side dish but also names a main
   ingredient ("Sheet Pan Lemon Chicken & Green Beans", "Pork Chops and Roasted Potatoes"), it is a
   main dish.
4. If the name says nothing, the **description** is checked the same way.
5. If neither does, ingredients such as chicken, beef or salmon suggest **Main Dishes**.
6. Otherwise there is no suggestion and the recipe stays uncategorised.

Keywords match whole words, also in plural form.

Examples: "Grandma's Buttermilk Pancakes" → Breakfast; "Classic Chili" → Soups & Stews;
"Turkey Pot Pie" → Main Dishes; "Creamy Mashed Potatoes" → Side Dishes; "Taco
Seasoning (Spice Mix)" → Seasonings & Spice Mixes; "Hot Apple Cider" → Drinks; "Homemade Laundry Soap" →
Household & DIY.

??? note "All keywords"

    | Category | Keywords |
    |---|---|
    | Household & DIY | goo b gone, goo gone, stain remover, counter cleaner, deodorant, cleaner, cleaning, detergent, laundry, soap, lotion, lip balm, bath salt, bath bomb, scrub, dishwasher, disinfectant, air freshener, candle, play dough, playdough, washing soda, body butter, toothpaste |
    | Seasonings & Spice Mixes | soup mix, dressing mix, seasoning mix, ranch seasoning, seasoning, spice mix, spice blend, rub, taco seasoning, everything bagel seasoning, chip seasoning |
    | Drinks | drink, smoothie, lemonade, limeade, tea, coffee, latte, cocoa, hot chocolate, punch, wassail, cider, shake, milkshake, cocktail, mocktail, juice, spiced milk, eggnog, horchata, agua fresca, kombucha, sangria |
    | Breakfast | griddle cake, yogurt, breakfast log, pancake, waffle, french toast, oatmeal, porridge, granola, omelet, omelette, frittata, breakfast, crepe, hash brown, breakfast hash, quiche, overnight oats, eggs benedict, scrambled eggs, shakshuka, breakfast burrito |
    | Desserts | treats, rice krispie, gingerbread, peanut butter cups, crust, pie crust, pie dough, candy, muddy buddies, snowball, cake, cookie, brownie, pie, cheesecake, frosting, icing, fudge, candy, pudding, crisp, crumble, cobbler, dessert, tart, ice cream, blondie, cupcake, truffle, mousse, brulee, fondant, toffee, caramel, custard, macaron, meringue, sorbet, trifle, bar, bars, snickerdoodle, shortbread, baklava, tiramisu, donut, doughnut, cinnamon roll, sweet roll, buttercream, ganache, cannoli, eclair, turnover, galette |
    | Breads | bread, biscuit, roll, rolls, bun, buns, sourdough, focaccia, tortilla, naan, pizza dough, bagel, loaf, cornbread, dough, brioche, challah, pita, flatbread, scone, muffin, muffins, pretzel, croissant, english muffin |
    | Soups & Stews | soup, stew, chili, chowder, bisque, gumbo, pho, ramen, broth, minestrone, goulash, posole, pozole |
    | Salads | salad, slaw, coleslaw, caprese |
    | Sauces & Condiments | applesauce, reduction, au jus, queso, condensed milk, sauce, dressing, gravy, mayo, mayonnaise, ketchup, pesto, vinaigrette, marinade, syrup, jam, jelly, compote, relish, pickles, pickled, chutney, aioli, glaze, salsa, dip, hummus, guacamole, curd, preserves, frosting |
    | Appetizers & Snacks | cheese ball, kettle corn, chex mix, fruit cups, pumpkin seeds, appetizer, bites, poppers, wings, nachos, snack, trail mix, deviled eggs, crackers, chips, dip, salsa, guacamole, hummus, bruschetta, crostini, pinwheels, sliders, meatballs, energy balls, popcorn, jerky |
    | Side Dishes | lentils, lentil, squash, carrots, broccoli, brussels sprouts, asparagus, vegetables, corn, side, mashed, roasted vegetables, rice, pilaf, risotto, beans, stuffing, mac and cheese, macaroni and cheese, potatoes, fries, green beans, corn on the cob, coleslaw, au gratin, scalloped |
    | Main Dishes | tilapia, cod, halibut, meatball, sloppy joe, crunchwrap, bowl, hash, chicken, beef, pork, steak, fish, salmon, shrimp, tuna, turkey, lamb, sausage, lasagna, casserole, tacos, taco, enchilada, enchiladas, curry, pasta, spaghetti, fettuccine, alfredo, burger, pizza, meatloaf, stir fry, stir-fry, roast, fajitas, burrito, quesadilla, pot pie, ham, ribs, brisket, pulled pork, gnocchi, ravioli, noodles, mostaccioli, ziti, chops, tenders, nuggets, kabob, kebab, sandwich, wrap, meatza |

## Categories on import

When you import recipes, any recipe **without a category** gets the suggested one. Recipes
that already have categories keep them.

- **Website, Text and Photo** imports always get the suggestion. You see it in the editor
  before saving and can change it.
- **File and Mealie** imports have the option **Sort recipes into categories** (on by
  default). Turn it off to import recipes exactly as they are.

## Organizing existing recipes

The **Organize recipes** screen (in **Settings** and in the recipe list's **⋮** menu) updates
all of your household's recipes at once. It works in two steps:

1. **Preview.** Choose what to do and tap **Preview**: Larder lists every recipe that would
   change, with its name, its new category and whether its ingredients would be tidied.
   Nothing is saved yet.
2. **Apply.** Saves the changes.

The options:

| Option | Effect |
|---|---|
| **Only recipes without a category** (default) | Adds the suggested category to recipes without one. |
| **All recipes (replace first category)** | Puts the suggested category first on every recipe for which there is a suggestion. Existing categories are kept after it (a duplicate of the suggestion is removed), so recipes may move to a different cookbook chapter. |
| **Don't change categories** | Leaves categories alone. |
| **Tidy ingredient text** (on by default) | Re-runs [ingredient tidying](editing.md#how-larder-tidies-ingredient-lines) on every recipe, for recipes saved or imported before it existed. |

Applying saves each changed recipe like an edit: it gets a new version, and other devices pick
it up at their next sync. In connected mode this needs a connection to the server.

!!! tip

    Run a preview first, especially with **All recipes**: the keyword rules are good but not
    perfect, and recipes you sorted by hand might move.

The API equivalent is [`POST /api/recipes/organize`](../reference/api.md#organizing-recipes).
