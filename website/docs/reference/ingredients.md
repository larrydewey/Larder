# Ingredient parsing & units

Every ingredient line is stored as the text (`original`) and a parse into **quantity**,
optional **quantity_max** (for ranges), **unit**, **food** and **note**. The parse drives
scaling, unit conversion, shopping-list merging, aisles and the cookbook index.

Two steps run on every recipe that is saved or imported, on the server and in the on-device
engine (`larder-core`):

1. **Parsing** (`parse.rs`): the line is split into its parts.
2. **Normalisation** (`food.rs`, `Ingredient::canonicalize`): the food name is tidied and, if
   the line was understood, `original` is rewritten in a standard form.

With [AI assist](../guide/ai.md) the API can also parse lines with a language model
(`POST /api/parse/ingredients` with `use_llm: true`).

## Grammar

A line is read left to right:

```text
[bullet] [quantity [range]] [-] [x] [(size)] [size unit container | unit[.]] [(size)] [of] food [(note)] [, note]
```

1. **Clean-up.** Whitespace is collapsed. Unicode fractions become ASCII (`1½` → `1 1/2`,
   `¾` → `3/4`; all of ½ ⅓ ⅔ ¼ ¾ ⅕ ⅖ ⅗ ⅘ ⅙ ⅚ ⅛ ⅜ ⅝ ⅞), and so do fractions typeset with
   superscript and subscript digits (`1 ¹/₂` → `1 1/2`, `²¹⁄₃₂` → `21/32`). A superscript or
   subscript digit that is not part of such a fraction is a footnote marker and is dropped
   (`chili powder²` → `chili powder`). The fraction slash `⁄` becomes `/`, tabs and
   non-breaking spaces become spaces, and en/em dashes and the minus sign become `-`. A
   leading bullet (`- `, `* `, `• `, `· `, `▢ `, `□ `, `☐ `) is dropped.
2. **Quantity.** One of:
    - an integer or decimal, with `.` or `,` as the decimal mark: `2`, `1.5`, `1,5`;
    - a fraction or mixed number: `3/4`, `1 1/2`;
    - optionally followed by a **range**: `2-3`, `2 to 3`, `2 or 3`. The upper value is kept
      only if it is larger than the lower one;
    - or a number word at the start: `one` … `twelve`, `dozen` (12), `half`, `a half`,
      `one half` (0.5), `a quarter` (0.25);
    - or `a` / `an` followed by a unit: `a pinch of salt` → 1 pinch.
3. A dash between the number and a unit is skipped (`4-ounces`), and so is a multiplier `x `
   or `× ` (`2 x 400g tins`).
4. A parenthesised size right after the quantity becomes part of the note:
   `1 (14 oz) can tomatoes` → 1 can, note "14 oz".
5. **Sized containers.** A weight or volume followed by a container (`can`, `jar`, `package`,
   `stick`, `bunch`, `head`) is read as *count × container* with the size in the note:
    - `10 ounce can pineapple` → 1 can, note "10 oz";
    - `1 - 8oz package cream cheese` → 1 package, note "8 oz";
    - `1 3-ounce package cream cheese` → 1 package, note "3 oz";
    - `2 8-ounce packages cream cheese` → 2 packages, note "8 oz".
6. **Unit.** Any alias from the [table below](#unit-aliases), matched longest first,
   case-insensitively except for the case-sensitive ones, and only as a whole word. A
   trailing `.` is swallowed (`tbsp.`). Units are **only recognised after a quantity**:
   "Head of lettuce" and "Cups of joy" stay plain text. After the unit, another parenthesised
   size and a leading `of` are skipped.
7. **Food and note.** The rest is the food. Text after the first comma **outside
   parentheses** is a note, so `butter (cold, cubed)` stays together. A trailing parenthesis
   becomes a note (`cream cheese (softened)`). The endings ` to taste`, ` optional`,
   ` (optional)`, ` for garnish`, ` for serving` and ` as needed` move into the note. Several
   notes are joined with `, `.
8. A line with no quantity and no unit is entirely food (plus any note).

## Normalisation

After parsing, the food phrase is normalised:

1. Stray bullets, dashes, trailing punctuation and footnote asterisks are removed.
2. A parenthetical remark (also an unclosed one left by a truncated line, as in
   `unsalted butter (cold`) moves to the note.
3. **Descriptor words** (preparation, temperature and size) move from the food to the note.
   **Size words and "fresh" come first**, each on its own; the others keep the author's order:
   "finely chopped fresh cilantro" → food "cilantro", note "fresh, finely chopped".
4. **Spelling** is fixed word by word, and everything is lower-cased except proper names and
   acronyms (`BBQ`).
5. **Synonyms** are replaced by one canonical name (matched against the whole remaining
   phrase).
6. The note is rebuilt as *descriptors, original note, remark*, without repeating anything.
7. If the line was understood (it has a quantity, or no unit at all, and any unit is a known
   one), `original` is **rewritten in standard form** from the parts: abbreviated, pluralised
   units, the food, then the note after a comma.

Lines that are not a single ingredient are left **exactly as written**: when the food contains
`:` (a sub-recipe or instruction such as `egg wash: 1 egg beaten with milk`), or when there is
no quantity but the food contains a digit (`Juice of 2 lemons`).

### Examples

| Line as typed or imported | Stored as | quantity | max | unit | food | note |
|---|---|---|---|---|---|---|
| `2 Tablespoons butter, melted` | `2 tbsp butter, melted` | 2 | | tbsp | butter | melted |
| `1 1/2 cups All Purpose Flour` | `1 1/2 cups all-purpose flour` | 1.5 | | cup | all-purpose flour | |
| `3 large Eggs, beaten` | `3 eggs, large, beaten` | 3 | | | eggs | large, beaten |
| `1 cup finely chopped fresh cilantro` | `1 cup cilantro, fresh, finely chopped` | 1 | | cup | cilantro | fresh, finely chopped |
| `Freshly ground black pepper` | `black pepper, freshly` | | | | black pepper | freshly |
| `3 cloves garlic, minced` | `3 cloves garlic, minced` | 3 | | clove | garlic | minced |
| `2-3 cloves garlic, minced` | `2–3 cloves garlic, minced` | 2 | 3 | clove | garlic | minced |
| `1 ¹/₂ cups Cheddar Cheese` | `1 1/2 cups Cheddar cheese` | 1.5 | | cup | Cheddar cheese | |
| `4 Tablespoon chili powder²` | `4 tbsp chili powder` | 4 | | tbsp | chili powder | |
| `4-ounces of shredded cheddar cheese` | `4 oz Cheddar cheese, shredded` | 4 | | oz | Cheddar cheese | shredded |
| `10 ounce can crushed pineapple` | `1 can pineapple, crushed, 10 oz` | 1 | | can | pineapple | crushed, 10 oz |
| `2 8-ounce packages cream cheese, softened` | `2 packages cream cheese, 8 oz, softened` | 2 | | package | cream cheese | 8 oz, softened |
| `1 (14 oz) can diced tomatoes` | `1 can tomatoes, diced, 14 oz` | 1 | | can | tomatoes | diced, 14 oz |
| `unsalted butter (cold, cubed)` | `unsalted butter, cold, cubed` | | | | unsalted butter | cold, cubed |
| `Salt and pepper to taste` | `salt and pepper, to taste` | | | | salt and pepper | to taste |
| `5 cc water` | `5 ml water` | 5 | | ml | water | |
| `500g plain flour` | `500 g all-purpose flour` | 500 | | g | all-purpose flour | |
| `a pinch of nutmeg` | `1 pinch nutmeg` | 1 | | pinch | nutmeg | |
| `2 T butter` | `2 tbsp butter` | 2 | | tbsp | butter | |
| `1 t vanilla` | `1 tsp vanilla extract` | 1 | | tsp | vanilla extract | |
| `Juice of 2 lemons` | `Juice of 2 lemons` *(unchanged)* | | | | Juice of 2 lemons | |
| `egg wash: 1 egg beaten with milk` | *(unchanged)* | | | | egg wash: 1 egg beaten with milk | |
| `Head of lettuce` | `head of lettuce` | | | | head of lettuce | |

### Descriptor words

These move from the food to the note, as do words ending in `-chopped`, `-diced`, `-minced`,
`-sliced` or `-grated` ("finely-diced"):

`chopped`, `diced`, `minced`, `sliced`, `grated`, `shredded`, `softened`, `melted`, `cubed`, `crushed`, `beaten`, `divided`, `peeled`, `seeded`, `halved`, `quartered`, `toasted`, `julienned`, `trimmed`, `rinsed`, `drained`, `zested`, `juiced`, `mashed`, `sifted`, `packed`, `cold`, `warm`, `chilled`, `fresh`, `freshly`, `finely`, `roughly`, `coarsely`, `thinly`, `lightly`, `loosely`, `firmly`, `large`, `medium`, `small`, `extra-large`, `jumbo`, `heaping`, `level`, `optional`, `cooked`, `room-temperature`

### Spelling fixes

??? note "Misspellings corrected (whole words)"

    | Written | Corrected to |
    |---|---|
    | tomatoe | tomato |
    | potatoe | potato |
    | jalepeno | jalapeño |
    | jalepenos | jalapeños |
    | jalapeno | jalapeño |
    | jalapenos | jalapeños |
    | cayanne | cayenne |
    | cayan | cayenne |
    | lemmon | lemon |
    | epsome | epsom |
    | worchestershire | worcestershire |
    | worcestshire | worcestershire |
    | parmesean | parmesan |
    | brocolli | broccoli |
    | brocoli | broccoli |
    | zuchini | zucchini |
    | zuccini | zucchini |
    | cinamon | cinnamon |
    | mozzerella | mozzarella |
    | mozarella | mozzarella |
    | vanila | vanilla |
    | tumeric | turmeric |
    | oregeno | oregano |
    | rasberry | raspberry |
    | rasberries | raspberries |
    | avacado | avocado |
    | avacados | avocados |
    | siracha | sriracha |
    | carmel | caramel |
    | expresso | espresso |
    | cardamon | cardamom |
    | habenero | habanero |
    | proscuitto | prosciutto |
    | pinapple | pineapple |

### Synonyms

| Any of these | Becomes |
|---|---|
| all purpose flour, ap flour, plain flour, all-purpose white flour, all purpose white flour, white flour | all-purpose flour |
| confectioners sugar, confectioners' sugar, confectioner's sugar, icing sugar, powdered sugar | powdered sugar |
| white sugar, granulated sugar, granulated white sugar, white granulated sugar, cane sugar | sugar |
| light brown sugar | light brown sugar |
| heavy whipping cream, whipping cream, heavy cream, double cream | heavy cream |
| bicarbonate of soda, bicarb, bicarb soda, sodium bicarbonate | baking soda |
| scallion, scallions, spring onion, spring onions, green onion, green onions | green onions |
| extra virgin olive oil, extra-virgin olive oil, evoo | extra-virgin olive oil |
| ground black pepper, black pepper, cracked black pepper, ground pepper | black pepper |
| ground cinnamon | cinnamon |
| ground cumin | cumin |
| ground nutmeg | nutmeg |
| vanilla, vanilla extract, pure vanilla extract, pure vanilla | vanilla extract |
| garlic clove, garlic cloves, clove garlic, cloves garlic, cloves of garlic, clove of garlic | garlic |
| egg, eggs, whole egg, whole eggs | eggs |
| corn starch, cornflour | cornstarch |
| kosher sea salt | kosher salt |
| table salt | salt |

### Capitalisation

Food names are lower case except acronyms (two or more capital letters, such as `BBQ`) and
these proper names, which are capitalised: 
parmesan, parmigiano, reggiano, romano, asiago, gruyère, gruyere, dijon, worcestershire, italian, french, greek, mexican, cajun, creole, thai, chinese, japanese, korean, indian, spanish, swiss, colby, kalamata, tabasco, sriracha, bisquick, karo, oreo, oreos, nutella, crisco, velveeta, philadelphia, frank's, cheddar. Multi-word names capitalised as a phrase: new mexico, new york, new zealand, old bay, rice krispies, hidden valley, granny smith, yukon gold, monterey jack, texas toast. So "new potatoes" stays lower case but "new mexico chile" becomes "New Mexico chile", and ordinary words such as "bay" or "old" stay lower case ("bay leaves") unless they are part of a name ("Old Bay seasoning").

### Food keys

To recognise the same food in different recipes, each food also has a **key**: accent-folded,
lower case, with every word made singular ("tomatoes" → "tomato", "berries" → "berry",
"peaches" → "peach", "bay leaves" → "bay leaf"). These words are never made singular:
molasses, hummus, asparagus, couscous, swiss, oats, grits, greens, sprinkles, noodles, brussels, chips, flakes, bits, lentils, beans, peas, chives, glass, bass, citrus, series, species, cheese, mayonnaise, chess, and words of three letters or fewer or ending in `ss`, `us` or `is`.
The key is used to merge shopping-list items, so "egg" and "eggs" merge.

### Section headings

In the app's editor a line starting with `#` is a section heading. When importing text,
a line is a heading if it starts with `#` or ends with `:` (shorter than 60 characters and not
starting with a digit), for example `For the dough:`.

### When structured fields are kept

If an ingredient is sent with structured fields (quantity, unit or food), they are kept and
only cleaned up and normalised (units mapped to canonical keys, the food tidied as above). The
text is parsed only when all three are missing. The editor sends changed lines with only
`original`, so they are re-parsed. To re-run normalisation on recipes saved before it existed,
use [Organizing recipes](../guide/organize.md).

## Unit aliases

The canonical key is what is stored in `unit`. Aliases are matched case-insensitively unless
marked case-sensitive, so `T` is a tablespoon but `t` is a teaspoon, and `C` and `c` are cups.

| Key | Kind | Size | System | Singular / plural | Aliases |
|---|---|---|---|---|---|
| `tsp` | volume | 4.92892 ml | US / imperial | tsp / tsp | `tsp`, `tsps`, `teaspoon`, `teaspoons`, `tea spoon`, `tea spoons`, `tspn`, `c. à c.`; case-sensitive: `t` |
| `tbsp` | volume | 14.7868 ml | US / imperial | tbsp / tbsp | `tbsp`, `tbsps`, `tbs`, `tbl`, `tbls`, `tablespoon`, `tablespoons`, `table spoon`, `table spoons`, `tblsp`, `c. à s.`, `el`; case-sensitive: `T`, `Tb` |
| `fl oz` | volume | 29.5735 ml | US / imperial | fl oz / fl oz | `fl oz`, `fl. oz`, `fl.oz`, `floz`, `fluid ounce`, `fluid ounces`, `fl ounce`, `fl ounces` |
| `cup` | volume | 236.588 ml | US / imperial | cup / cups | `cup`, `cups`, `c`, `cp`, `tasse`, `tassen`; case-sensitive: `C` |
| `pint` | volume | 473.176 ml | US / imperial | pint / pints | `pint`, `pints`, `pt`, `pts` |
| `quart` | volume | 946.353 ml | US / imperial | quart / quarts | `quart`, `quarts`, `qt`, `qts` |
| `gallon` | volume | 3785.41 ml | US / imperial | gallon / gallons | `gallon`, `gallons`, `gal`, `gals` |
| `ml` | volume | 1.0 ml | metric | ml / ml | `ml`, `mls`, `milliliter`, `milliliters`, `millilitre`, `millilitres`, `cc`, `cc.`; case-sensitive: `mL` |
| `cl` | volume | 10.0 ml | metric | cl / cl | `cl`, `centiliter`, `centiliters`, `centilitre`, `centilitres` |
| `dl` | volume | 100.0 ml | metric | dl / dl | `dl`, `deciliter`, `deciliters`, `decilitre`, `decilitres` |
| `l` | volume | 1000.0 ml | metric | l / l | `l`, `liter`, `liters`, `litre`, `litres`, `ltr`, `ltrs`, `lt`; case-sensitive: `L` |
| `oz` | mass | 28.3495 g | US / imperial | oz / oz | `oz`, `ozs`, `ounce`, `ounces`, `oz.` |
| `lb` | mass | 453.592 g | US / imperial | lb / lb | `lb`, `lbs`, `pound`, `pounds`, `lb.`, `lbs.` |
| `mg` | mass | 0.001 g | metric | mg / mg | `mg`, `milligram`, `milligrams`, `milligramme`, `milligrammes` |
| `g` | mass | 1.0 g | metric | g / g | `g`, `gr`, `grs`, `gram`, `grams`, `gramme`, `grammes`, `gm`, `gms` |
| `kg` | mass | 1000.0 g | metric | kg / kg | `kg`, `kgs`, `kilogram`, `kilograms`, `kilo`, `kilos`, `kilogramme`, `kilogrammes` |
| `pinch` | count | – | – | pinch / pinches | `pinch`, `pinches`, `pn` |
| `dash` | count | – | – | dash / dashes | `dash`, `dashes` |
| `clove` | count | – | – | clove / cloves | `clove`, `cloves` |
| `can` | count | – | – | can / cans | `can`, `cans`, `tin`, `tins` |
| `jar` | count | – | – | jar / jars | `jar`, `jars` |
| `package` | count | – | – | package / packages | `package`, `packages`, `pkg`, `pkgs`, `packet`, `packets`, `pack`, `packs`, `box`, `boxes`, `bag`, `bags`, `envelope`, `envelopes` |
| `slice` | count | – | – | slice / slices | `slice`, `slices` |
| `piece` | count | – | – | piece / pieces | `piece`, `pieces`, `pc`, `pcs` |
| `stick` | count | – | – | stick / sticks | `stick`, `sticks` |
| `bunch` | count | – | – | bunch / bunches | `bunch`, `bunches`, `bn` |
| `sprig` | count | – | – | sprig / sprigs | `sprig`, `sprigs` |
| `handful` | count | – | – | handful / handfuls | `handful`, `handfuls` |
| `head` | count | – | – | head / heads | `head`, `heads` |
| `drop` | count | – | – | drop / drops | `drop`, `drops` |

Any other unit text (for example `knob`) is kept as free text and never converted.

## Conversion

Conversion only happens between units of the same kind (volume ↔ volume, mass ↔ mass), never
for counts or unknown units, and never between volume and weight. The same rules
(`units.rs`, `format.rs`) are used everywhere: on the server, in the PDF cookbook, in the
on-device engine, and in the app's recipe page and cook mode (`app/lib/util/units.dart` is a
line-by-line port), so amounts are identical on screen and in print.

| Target | Rule |
|---|---|
| Original | No change |
| Metric, from a US unit | Volume: `ml`, or `l` from 1000 ml. Mass: `g`, or `kg` from 1000 g. |
| Imperial, from a metric unit | Volume: `gallon` from 16 cups, `quart` from 4 cups, `cup` from ¼ cup, `tbsp` from about 14.3 ml, else `tsp`. Mass: `lb` from 16 oz, else `oz`. |
| Same system already | No change |

Metric results are then rounded for the kitchen:

| Amount | Rounded to |
|---|---|
| g or ml, 100 or more | nearest 5 (`473 ml` → `475 ml`) |
| g or ml, 10 to 100 | whole number |
| g or ml, under 10 | one decimal |
| kg or l | two decimals |

Ranges are converted with the same factor as their lower bound. Factors are those in the
[unit table](#unit-aliases) (1 cup = 236.588 ml, 1 tbsp = 14.7868 ml, 1 tsp = 4.92892 ml,
1 oz = 28.3495 g, 1 lb = 453.592 g, …).

### Number formatting

Quantities are written as kitchen fractions where that reads naturally: the fractional part
is matched to eighths and thirds (1/8, 1/4, 1/3, 3/8, 1/2, 5/8, 2/3, 3/4, 7/8) within 0.02;
within 0.03 of a whole number it is rounded to it; otherwise up to two decimals are shown.
From 20 upwards, at most one decimal. Ranges use an en dash: `2–3`. The app and the PDF
cookbook print the fraction as a single character (`1½`); shopping-list text uses ASCII
(`1 1/2`).

Units are pluralised with the forms in the table above when the amount (the upper bound for
ranges) is more than 1.

## Aisles

Shopping-list items are put in one of twelve aisles, always shown in this order:

1. Produce
2. Meat & Seafood
3. Dairy & Eggs
4. Bakery
5. Pantry
6. Baking
7. Spices & Seasonings
8. Canned & Jarred
9. Frozen
10. Beverages
11. Household
12. Other

### Classification

The food name is accent-folded, lower-cased and split into words. Each keyword is matched as
**whole words**; the last word of a keyword may also appear in plural form (`egg` matches
"eggs" but not "eggplant"; `berry` matches "berries"; `leaf` matches "leaves"). Of all the
keywords that match:

1. the keyword with **the most words wins** ("cream of tartar" beats "cream", "garlic powder"
   beats "garlic", "coconut milk" beats "milk");
2. among equally long keywords, the **earlier rule** in the table below wins.

Nothing matching gives **Other**. Shopping items measured in a **can** or **jar** always go to **Canned & Jarred**, whatever the food (so "1 can tomatoes, diced" is not filed under Produce). You can always change an item's aisle with **Edit item**.

??? note "All keywords, in rule order"

    | Order | Aisle | Keywords |
    |---|---|---|
    | 1 | Frozen | frozen, ice cream, puff pastry, phyllo, filo |
    | 2 | Canned & Jarred | barbecue sauce, bbq sauce, artichoke heart, pico de gallo, enchilada sauce, marinara, pasta sauce, alfredo sauce, canned, can of, tinned, coconut milk, tomato paste, tomato sauce, crushed tomatoes, diced tomatoes, passata, broth, stock, bouillon, beans, chickpeas, lentils, salsa, pesto, olives, capers, pickles, jam, jelly, peanut butter, tahini, mayonnaise, mayo, ketchup, mustard, soy sauce, fish sauce, worcestershire, hot sauce, sriracha, honey, maple syrup, molasses, vinegar |
    | 3 | Spices & Seasonings | rub, ranch seasoning, taco seasoning, salt, pepper flakes, peppercorn, black pepper, white pepper, cumin, paprika, turmeric, cinnamon, nutmeg, ground clove, whole clove, cardamom, coriander seed, ground coriander, chili powder, chilli powder, cayenne, oregano, dried basil, dried thyme, thyme dried, bay leaf, bay leaves, garam masala, curry powder, allspice, ginger powder, ground ginger, garlic powder, onion powder, italian seasoning, seasoning, spice, fennel seed, mustard seed, saffron, star anise, sumac, za'atar, herbes de provence, vanilla |
    | 4 | Meat & Seafood | hamburger, ground beef, pepperoni, meatball, chicken, beef, pork, lamb, turkey, bacon, sausage, ham, prosciutto, salami, chorizo, steak, mince, ground meat, veal, duck, fish, salmon, tuna, cod, shrimp, prawn, crab, lobster, scallop, mussel, clam, anchovy, anchovies, tilapia, halibut, trout |
    | 5 | Dairy & Eggs | milk, buttermilk, butter, cream, cheese, parmesan, mozzarella, cheddar, feta, ricotta, yogurt, yoghurt, egg, sour cream, crème fraîche, creme fraiche, half-and-half, ghee, mascarpone |
    | 6 | Baking | cake mix, pie crust, corn syrup, karo, masa harina, extract, toffee, caramel, marshmallow, graham cracker, flour, sugar, baking powder, baking soda, bicarbonate, yeast, cocoa, chocolate chip, chocolate, cornstarch, corn starch, cornflour, gelatin, sprinkles, food coloring, powdered sugar, icing sugar, brown sugar, shortening, almond extract, cream of tartar |
    | 7 | Bakery | hamburger bun, hot dog bun, slider bun, bread, baguette, bun, roll, tortilla, pita, naan, croissant, bagel, brioche |
    | 8 | Produce | artichoke, onion, garlic, shallot, scallion, green onion, leek, tomato, potato, carrot, celery, pepper, chili, chile, jalapeño, jalapeno, lettuce, spinach, kale, cabbage, broccoli, cauliflower, zucchini, courgette, squash, pumpkin, cucumber, eggplant, aubergine, mushroom, avocado, lemon, lime, orange, apple, banana, berry, berries, blueberry, strawberry, raspberry, blackberry, cranberry, grape, pear, peach, mango, pineapple, cherry, cherries, melon, basil, parsley, cilantro, coriander, mint, dill, rosemary, thyme, sage, chive, ginger, corn, pea, bean sprout, green bean, asparagus, beet, radish, fennel, sweet potato, yam, arugula, rocket, herb, fruit, vegetable |
    | 9 | Pantry | millet, oreo, tortilla chip, pretzel, rice, pasta, spaghetti, noodle, macaroni, penne, oats, oatmeal, quinoa, couscous, bulgur, cereal, oil, nuts, almond, walnut, pecan, cashew, peanut, pistachio, seed, raisin, dried fruit, breadcrumb, panko, cracker, chips, polenta, cornmeal, lentil, barley |
    | 10 | Beverages | water, juice, wine, beer, coffee, tea, soda, sparkling, rum, vodka, brandy, whiskey, liqueur |
    | 11 | Household | foil, parchment, paper towel, plastic wrap, toothpick, skewer, napkin, soap |

The aisle is classified from the normalised food name, so preparation words that moved to the
note do not count: "1 can tomatoes, diced" is classified from "tomatoes" (Produce). Change the
aisle on the item if you buy it canned.

An aisle you type that matches a standard aisle (ignoring case) is normalised to its spelling.
Other aisle names are allowed and grouped as their own heading after the standard ones.

## Shopping-list merging

The same function (`larder_core::shopping::merge_recipes`) is used by the server and by the
engine, so lists behave identically in both modes. For each ingredient of each recipe added:

1. **Food**: the parsed food, or the whole original line when there is no parsed food. Lines
   with neither are skipped.
2. **Scale**: `servings ÷ yield` when both are given and positive, else 1.
3. **Quantity**: the upper bound of a range if there is one, else the quantity, times the
   scale. No quantity stays no quantity.
4. **Find a match**: the first **unchecked** item with the same [food key](#food-keys)
   (accent-folded, lower case, singular, so "egg" and "eggs" match), and whose unit is
   **compatible**:
    - both without unit;
    - the same unit, or two units of the same kind (both volume, or both mass) that can be
      converted;
    - one with a unit and one without, only if one of the two has no quantity.
5. **Merge**: the quantity is converted into the existing item's unit and added (if the
   existing item had no unit, it takes the new one). The recipe is added to the item's
   `recipe_ids`.
6. **Otherwise** a new item is appended at the end with its aisle classified from the food.
7. When a quantity is added to an existing item, the total is tidied with `tidy_amount`: it is
   expressed in a sensible unit of the same system (US volume: tsp below 14 ml, tbsp below 118 ml
   (½ cup), cups below about 1 gallon, then quarts; fl oz stays fl oz; US mass: oz below 1 lb, then
   lb; metric: ml/l and g/kg switching at 1000) and rounded up to a step of ¼ tsp, ½ tbsp, ¼ cup,
   ¼ quart, ½ fl oz, ½ oz, ¼ lb, 5 g/ml (10 above 100), or 0.1 kg/l. Count and free-text units are
   only rounded to three decimals. `display` is then regenerated (for example
   `3 1/4 cups milk`).

Items you add by hand never have a recipe attached but are merged into like any other
unchecked item. Checked items are never merged into, so ticking something off and adding the
recipe again gives a fresh, unchecked item.
