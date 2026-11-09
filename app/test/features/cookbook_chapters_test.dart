import 'package:flutter_test/flutter_test.dart';
import 'package:larder/features/cookbook/cookbook_chapters.dart';
import 'package:larder/features/cookbook/cookbook_screen.dart';
import 'package:larder/models/recipe.dart';
import 'package:material_ui/material_ui.dart';

import '../support/app_harness.dart';
import '../support/fakes.dart';

Recipe r(String name, [List<String> cats = const [], List<String> tags = const []]) =>
    Recipe(id: name.toLowerCase(), name: name, categories: cats, tags: tags);

void main() {
  // Mirrors the tests of group_recipes in server/crates/cookbook/src/book.rs.
  test('one chapter per recipe: first category, merged case-insensitively, Other last', () {
    final rs = [
      r('Zucchini Bread', ['Breads', 'Breakfast']),
      r('Apple Pie', ['desserts']),
      r('Pancakes', ['Breakfast']),
      r('Mystery Stew'),
      r('Brownies', ['Desserts']),
      r('éclairs', ['Desserts']),
    ];
    // Larder categories in meal order (Breakfast before Breads).
    expect(cookbookChapters(rs, 'category'), ['Breakfast', 'Breads', 'Desserts', otherChapter]);
  });

  test('later categories do not create chapters; blanks are skipped', () {
    final rs = [
      r('A', ['  ', 'mains', 'Sides']),
      r('B', ['Mains']),
    ];
    expect(cookbookChapters(rs, 'category'), ['Mains']);
  });

  test('first-seen spelling, tidied like chapter_label', () {
    expect(
      cookbookChapters([
        r('A', ['  main   dishes ']),
        r('B', ['Main Dishes']),
      ], 'category'),
      ['Main dishes'],
    );
    expect(
      cookbookChapters([
        r('A', ['BBQ']),
        r('B', ['bbq']),
      ], 'category'),
      ['BBQ'],
    );
    expect(
      cookbookChapters([
        r('A', ['ñoquis']),
      ], 'category'),
      ['Ñoquis'],
    );
  });

  test('other names sort by the server sort key (accents folded, leading symbols ignored)', () {
    final rs = [
      r('1', ['Zuppe']),
      r('2', ['"Apps"']),
      r('3', ['Éclairs']),
      r('4', ['cocktails']),
    ];
    expect(cookbookChapters(rs, 'category'), ['"Apps"', 'Cocktails', 'Éclairs', 'Zuppe']);
  });

  test('Larder categories come first in meal order (book.rs suggested_categories_use_meal_order)', () {
    final rs = [
      r('Cake', ['Desserts']),
      r('Pancakes', ['Breakfast']),
      r('Tea', ['Drinks']),
      r('Odd', ['Zebra Food']),
      r('Soup', ['Soups & Stews']),
      r('Apple Thing', ['Apples']),
    ];
    expect(cookbookChapters(rs, 'category'), [
      'Breakfast',
      'Soups & Stews',
      'Desserts',
      'Drinks',
      'Apples',
      'Zebra Food',
    ]);
    // Matched ASCII case-insensitively; the first-seen spelling is kept.
    expect(
      cookbookChapters([
        r('A', ['DESSERTS']),
        r('B', ['main dishes']),
        r('C', ['Aardvark']),
      ], 'category'),
      ['Main dishes', 'DESSERTS', 'Aardvark'],
    );
  });

  test('book.rs respects_chapter_order: remaining chapters', () {
    // The server puts chapter_order names first and sorts the rest like this.
    final rs = [
      r('A', ['Mains']),
      r('B', ['Breakfast']),
      r('C', ['Desserts']),
      r('D', ['Appetizers']),
      r('E'),
    ];
    expect(cookbookChapters(rs, 'category'), ['Breakfast', 'Desserts', 'Appetizers', 'Mains', otherChapter]);
  });

  test('fallback chapters', () {
    expect(cookbookChapters([r('Soup'), r('Bread')], 'category'), [allChapter]);
    expect(
      cookbookChapters([
        r('Soup', ['Mains']),
      ], 'none'),
      [allChapter],
    );
    expect(cookbookChapters(<Recipe>[], 'category'), isEmpty);
    expect(cookbookChapters(<Recipe>[], 'none'), isEmpty);
    // A category literally named "other recipes" merges with the catch-all.
    expect(
      cookbookChapters([
        r('A', ['other recipes']),
        r('B'),
        r('C', ['Mains']),
      ], 'category'),
      ['Mains', 'Other recipes'],
    );
  });

  test('group by tag', () {
    final rs = [
      r('Soup', ['Mains'], ['winter', 'quick']),
      r('Salad', ['Sides'], ['Summer']),
      r('Toast'),
    ];
    expect(cookbookChapters(rs, 'tag'), ['Summer', 'Winter', otherChapter]);
  });

  testWidgets('chapter list follows the recipe selection and grouping', (tester) async {
    tester.view.physicalSize = const Size(900, 3000);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    final store = await tester.runAsync(memoryStore);
    await tester.runAsync(() async {
      await store!.putRecipe(r('Soup', ['Mains'], ['winter']));
      await store.putRecipe(r('Cake', ['desserts', 'Mains'], ['Sweet']));
      await store.putRecipe(r('Toast'));
    });
    final server = FakeServer()..on('GET', '/api/export/cookbook/themes', (_) => json([]));
    await tester.pumpWidget(wrap(const CookbookScreen(), testOverrides(store!, server)));
    for (var i = 0; i < 5; i++) {
      await tester.runAsync(() => Future<void>.delayed(const Duration(milliseconds: 50)));
      await tester.pump(const Duration(milliseconds: 50));
    }

    List<String> chapters() => tester
        .widgetList<ListTile>(find.descendant(of: find.byType(ReorderableListView), matching: find.byType(ListTile)))
        .map((t) => (t.title! as Text).data!)
        .toList();

    await tester.tap(find.text('Custom chapter order'));
    await tester.pump();
    expect(chapters(), ['Desserts', 'Mains', otherChapter]);

    await tester.tap(find.text('By tag'));
    await tester.pump();
    expect(chapters(), ['Sweet', 'Winter', otherChapter]);

    // Only "Toast" selected: nothing has a tag -> one "Recipes" chapter.
    await tester.tap(find.textContaining('Selected ('));
    await tester.pump();
    expect(chapters(), isEmpty);
    await tester.tap(find.text('Choose recipes'));
    await tester.pumpAndSettle();
    await tester.tap(find.widgetWithText(CheckboxListTile, 'Toast'));
    await tester.pump();
    await tester.tap(find.text('Done'));
    await tester.pumpAndSettle();
    expect(chapters(), [allChapter]);

    // Swap Toast for Soup (same selection size).
    await tester.tap(find.textContaining('Change selection'));
    await tester.pumpAndSettle();
    await tester.tap(find.widgetWithText(CheckboxListTile, 'Toast'));
    await tester.tap(find.widgetWithText(CheckboxListTile, 'Soup'));
    await tester.pump();
    await tester.tap(find.text('Done'));
    await tester.pumpAndSettle();
    expect(chapters(), ['Winter']);

    await tester.tap(find.text('By category'));
    await tester.pump();
    expect(chapters(), ['Mains']);
  });
}
