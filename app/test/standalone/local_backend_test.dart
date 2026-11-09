import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:larder/api/api_client.dart';
import 'package:larder/data/app_mode.dart';
import 'package:larder/data/backend.dart';
import 'package:larder/data/local_store.dart';
import 'package:larder/data/outbox_op.dart';
import 'package:larder/models/organize.dart';
import 'package:larder/data/providers.dart';
import 'package:larder/models/cookbook.dart';
import 'package:larder/models/meal_plan.dart';
import 'package:larder/models/recipe.dart';
import 'package:larder/standalone/standalone.dart';
import 'package:larder/standalone/standalone_io.dart';

import '../support/fakes.dart';

void main() {
  late LocalStore store;
  late FakeEngine engine;
  late Directory dir;
  late ProviderContainer c;
  late FakeServer server;

  setUp(() async {
    store = await memoryStore();
    engine = FakeEngine();
    dir = await Directory.systemTemp.createTemp('larder-test');
    server = FakeServer();
    final support = IoStandaloneSupport(engine: engine, root: () async => dir, httpClient: server.client);
    c = ProviderContainer(
      overrides: [
        localStoreProvider.overrideWithValue(store),
        apiClientProvider.overrideWithValue(ApiClient(baseUrl: 'http://srv', httpClient: server.client)),
        platformServicesEnabledProvider.overrideWithValue(false),
        bootstrapProvider.overrideWithValue(const BootstrapData(mode: AppMode.standalone)),
        standaloneSupportProvider.overrideWithValue(support),
      ],
    );
  });

  tearDown(() async {
    c.dispose();
    await dir.delete(recursive: true);
  });

  test('standalone mode picks the local backend', () {
    expect(c.read(recipeRepositoryProvider).runtimeType.toString(), 'LocalRecipeRepository');
    expect(c.read(importServiceProvider).runtimeType.toString(), 'LocalImportService');
  });

  test('save normalises through the engine and never touches the network', () async {
    final repo = c.read(recipeRepositoryProvider);
    final o = await repo.save(
      Recipe(id: 'r1', name: '  Pancakes ', ingredients: const [Ingredient.unparsed('2 cup flour')]),
      isNew: true,
    );
    expect(o.kind, OutcomeKind.applied);
    final r = (await store.recipe('r1'))!;
    expect(r.name, 'Pancakes');
    expect(r.slug, 'pancakes');
    expect(r.searchText, 'pancakes');
    expect(r.ingredients.single.food, 'flour');
    expect(r.ingredients.single.quantity, 2);
    expect(r.version, 1);
    expect(await store.opCount(), 0);
    expect(server.requests, isEmpty);

    await repo.save(r.copyWith(name: 'Fluffy pancakes'), isNew: false);
    expect((await store.recipe('r1'))!.version, 2);
  });

  test('images are processed into local files and removed with the recipe', () async {
    final repo = c.read(recipeRepositoryProvider);
    await repo.save(Recipe(id: 'r1', name: 'Pie'), isNew: true);
    final r = await repo.setImage('r1', Uint8List.fromList([1, 2, 3]), 'pie.jpg');
    expect(r.localImages.keys, containsAll(['thumb', 'medium', 'full']));
    expect(File(r.localImage('thumb')!).existsSync(), isTrue);
    final dup = await repo.duplicate('r1');
    expect(dup.name, 'Pie (copy)');
    expect(File(dup.localImage('full')!).existsSync(), isTrue);
    await repo.delete(r);
    expect(File(r.localImage('thumb')!).existsSync(), isFalse);
    expect(File(dup.localImage('full')!).existsSync(), isTrue);
  });

  test('shopping: engine parsing + merge of recipes and meal plan', () async {
    final recipes = c.read(recipeRepositoryProvider);
    await recipes.save(
      Recipe(id: 'r1', name: 'Soup', yieldQuantity: 2, ingredients: const [Ingredient.unparsed('2 cup stock')]),
      isNew: true,
    );
    final shopping = c.read(shoppingRepositoryProvider);
    final list = await shopping.createList('Weekly');
    await shopping.addItem(list, '3 lbs apples');
    var l = (await store.shoppingList(list.id))!;
    expect(l.items!.single.food, 'apples');
    expect(l.items!.single.aisle, 'Produce');

    await shopping.addRecipes(list.id, [(recipeId: 'r1', servings: 4)]);
    l = (await store.shoppingList(list.id))!;
    expect(l.items!.map((i) => i.food), ['apples', 'stock']);
    expect(l.itemCount, 2);

    await c.read(mealPlanRepositoryProvider).create(MealPlanEntry(id: 'm1', date: '2026-11-10', recipeId: 'r1'));
    await shopping.addMealPlan(list.id, DateTime(2026, 11, 9), DateTime(2026, 11, 15));
    l = (await store.shoppingList(list.id))!;
    expect(l.items!.length, 3);

    await shopping.setChecked(l.items!.first, true);
    await shopping.clearChecked((await store.shoppingList(list.id))!);
    expect((await store.shoppingList(list.id))!.items!.length, 2);
  });

  test('bulk file import reports progress, skips duplicates and saves images', () async {
    final img = File('${dir.path}/extracted.jpg')..writeAsBytesSync([9, 9, 9]);
    await c.read(recipeRepositoryProvider).save(Recipe(id: 'x', name: 'Existing'), isNew: true);
    engine.overrides['import_file'] = (args) => {
      'source': 'mealie',
      'recipes': [
        {
          'data': {'name': 'Existing'},
          'image_path': null,
        },
        {
          'data': {
            'name': 'New one',
            'ingredients': [
              {'original': '1 cup rice'},
            ],
          },
          'image_path': img.path,
          'favorite': true,
        },
      ],
      'errors': ['Recipe "Broken": missing name'],
    };
    final jobs = await c
        .read(importServiceProvider)
        .importFile(Uint8List.fromList(utf8.encode('zip')), 'export.zip', source: 'auto')
        .toList();
    final last = jobs.last;
    expect(last.status, 'done');
    expect(last.source, 'mealie');
    expect(last.imported, 1);
    expect(last.skipped, 1);
    expect(last.errors, contains('Recipe "Broken": missing name'));
    final saved = (await store.recipes()).firstWhere((r) => r.name == 'New one');
    expect(saved.isFavorite, isTrue);
    expect(saved.localImages, isNotEmpty);
    expect((await c.read(importServiceProvider).history()).single.id, last.id);
  });

  test('bulk import uses the server duplicate rule (source URL or folded name)', () async {
    final repo = c.read(recipeRepositoryProvider);
    await repo.save(Recipe(id: 'a', name: 'Crème  Brûlée'), isNew: true);
    await repo.save(Recipe(id: 'b', name: 'Soup', sourceUrl: 'https://ex.com/soup'), isNew: true);
    engine.overrides['import_file'] = (args) => {
      'source': 'mealie',
      'recipes': [
        {
          'data': {'name': 'CREME brulee '}, // folded name matches
        },
        {
          'data': {'name': 'Totally different', 'source_url': 'https://ex.com/soup'}, // URL matches
        },
        {
          'data': {'name': 'Soup II', 'source_url': 'https://ex.com/soup-2'},
        },
        {
          'data': {'name': 'soup ii'}, // duplicate of the one imported just before
        },
        {
          'data': {'name': 'Cafe', 'source_url': 'https://ex.com/cafe'},
        },
      ],
      'errors': [],
    };
    final last = await c
        .read(importServiceProvider)
        .importFile(Uint8List.fromList(utf8.encode('zip')), 'export.zip', source: 'auto')
        .last;
    expect(last.imported, 2);
    expect(last.skipped, 3);
    final names = (await store.recipes()).map((r) => r.name).toSet();
    expect(names, containsAll(['Soup II', 'Cafe']));
    expect(names, isNot(contains('Totally different')));
  });

  group('organize (engine, server rules)', () {
    setUp(() async {
      final parsed = [const Ingredient(quantity: 1, unit: 'cup', food: 'stock', original: '1 cup stock')];
      await store.putRecipe(Recipe(id: 'a', name: 'Tomato Soup', ingredients: parsed));
      await store.putRecipe(Recipe(id: 'b', name: 'Chocolate cake', categories: const ['Baking'], ingredients: parsed));
      await store.putRecipe(
        Recipe(id: 'c', name: 'Plain thing', ingredients: const [Ingredient.unparsed('2 cup stock')]),
      );
      await store.putRecipe(Recipe(id: 'd', name: 'Apple pie', categories: const ['desserts', 'Fruit']));
      await store.putRecipe(Recipe(id: 'e', name: 'Pancakes', categories: const ['Breakfast']));
    });

    Future<OrganizeResult> run(CategorizeMode mode, {bool normalize = true, bool dryRun = true}) =>
        c.read(recipeRepositoryProvider).organize(categorize: mode, normalize: normalize, dryRun: dryRun);

    test('uncategorized: preview fills only empty categories and reports tidied ingredients', () async {
      final res = await run(CategorizeMode.uncategorized);
      final byId = {for (final ch in res.changes) ch.id: ch};
      expect(byId.keys, unorderedEquals(['a', 'c']));
      expect(
        (byId['a']!.categoryBefore, byId['a']!.categoryAfter, byId['a']!.ingredientsChanged),
        (null, 'Soups & Stews', false),
      );
      expect((byId['c']!.categoryAfter, byId['c']!.ingredientsChanged), (null, true));
      expect((res.recipesChanged, res.categorized, res.normalized), (2, 1, 1));
      // Dry run: nothing saved.
      expect((await store.recipe('a'))!.categories, isEmpty);
      expect((await store.recipe('c'))!.ingredients.single.parsed, isFalse);
    });

    test('all: suggestion goes first, replacing a case-insensitive duplicate', () async {
      final res = await run(CategorizeMode.all, normalize: false, dryRun: false);
      expect(res.changes.map((ch) => ch.id), unorderedEquals(['a', 'b', 'd']));
      expect((await store.recipe('b'))!.categories, ['Desserts', 'Baking']);
      expect((await store.recipe('d'))!.categories, ['Desserts', 'Fruit']);
      expect((await store.recipe('a'))!.categories, ['Soups & Stews']);
      expect((await store.recipe('e'))!.version, isNull); // unchanged, not re-saved
      expect(res.byCategory.map((g) => g.key), containsAll(['Desserts', 'Soups & Stews']));
    });

    test('none: only ingredient tidying', () async {
      final res = await run(CategorizeMode.none, dryRun: false);
      expect(res.changes.map((ch) => ch.id), ['c']);
      expect(res.categorized, 0);
      final saved = (await store.recipe('c'))!;
      expect(saved.ingredients.single.food, 'stock');
      expect(saved.categories, isEmpty);
      expect(engine.calls, isNot(contains('suggest_categories')));
    });
  });

  test('bulk import sorts uncategorized recipes into categories unless switched off', () async {
    engine.overrides['import_file'] = (args) => {
      'source': 'mealie',
      'recipes': [
        {
          'data': {'name': 'Lentil soup'},
        },
        {
          'data': {
            'name': 'Beef stew',
            'categories': ['Winter'],
          },
        },
      ],
      'errors': [],
    };
    final imports = c.read(importServiceProvider);
    await imports.importFile(Uint8List.fromList([1]), 'x.zip', source: 'auto').last;
    var byName = {for (final r in await store.recipes()) r.name: r};
    expect(byName['Lentil soup']!.categories, ['Soups & Stews']);
    expect(byName['Beef stew']!.categories, ['Winter']);

    for (final r in await store.recipes()) {
      await store.deleteRecipe(r.id);
    }
    await imports.importFile(Uint8List.fromList([1]), 'x.zip', source: 'auto', categorize: false).last;
    byName = {for (final r in await store.recipes()) r.name: r};
    expect(byName['Lentil soup']!.categories, isEmpty);
  });

  test('previews carry the suggested category for the editor', () async {
    engine.overrides['import_text'] = (args) => {'name': 'Pumpkin soup', 'ingredients': [], 'instructions': []};
    final p = await c.read(importServiceProvider).previewText('Pumpkin soup ...');
    expect(p.recipe.categories, ['Soups & Stews']);
    engine.overrides['import_text'] = (args) => {
      'name': 'Pumpkin soup',
      'categories': ['Autumn'],
      'ingredients': [],
      'instructions': [],
    };
    expect((await c.read(importServiceProvider).previewText('x')).recipe.categories, ['Autumn']);
  });

  test('preview text uses heuristic parser unless AI requested', () async {
    final p = await c.read(importServiceProvider).previewText('Some recipe');
    expect(p.recipe.name, 'From text');
    expect(p.recipe.id, isEmpty);
    await expectLater(c.read(importServiceProvider).previewText('x', useLlm: true), throwsA(anything));
  });

  test('cookbook render and backup go through the engine with local image paths', () async {
    final repo = c.read(recipeRepositoryProvider);
    await repo.save(Recipe(id: 'r1', name: 'A'), isNew: true);
    await repo.setImage('r1', Uint8List.fromList([1]), 'a.jpg');
    await repo.save(Recipe(id: 'r2', name: 'B'), isNew: true);
    final cookbook = c.read(cookbookServiceProvider);
    expect((await cookbook.themes()).single.id, 'classic');
    final pdf = utf8.decode(await cookbook.renderCookbook(CookbookOptions(recipeIds: ['r1'])));
    expect(pdf, startsWith('render_cookbook:'));
    expect(pdf, contains('r1'));
    expect(pdf, isNot(contains('"r2"')));
    expect(pdf, contains('-full.jpg'));
    final zip = utf8.decode(await cookbook.exportBackup());
    expect(zip, contains('"favorite":false'));
  });

  test('disconnect download: remote images become local files', () async {
    final support = c.read(standaloneSupportProvider)!;
    await store.putRecipe(Recipe(id: 'r1', name: 'A', imageUrl: '/api/images/i1'));
    server.on('GET', '/api/images/i1', (r) => json([1, 2, 3]));
    final progress = <int>[];
    await support.downloadImages(store, c.read(apiClientProvider), (d, t, m) => progress.add(d));
    final r = (await store.recipe('r1'))!;
    expect(r.imageUrl, isNull);
    expect(File(r.localImage('medium')!).existsSync(), isTrue);
    expect(progress, [0, 1]);
  });
}
