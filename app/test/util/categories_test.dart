import 'package:flutter_test/flutter_test.dart';
import 'package:larder/util/categories.dart';

void main() {
  test('category list mirrors larder_core::categorize::CATEGORIES', () {
    expect(larderCategories, [
      'Breakfast',
      'Appetizers & Snacks',
      'Soups & Stews',
      'Salads',
      'Main Dishes',
      'Side Dishes',
      'Breads',
      'Desserts',
      'Drinks',
      'Sauces & Condiments',
      'Seasonings & Spice Mixes',
      'Household & DIY',
    ]);
  });

  test('rank is ASCII case-insensitive like eq_ignore_ascii_case', () {
    expect(categoryRank('breakfast'), 0);
    expect(categoryRank('HOUSEHOLD & diy'), 11);
    expect(categoryRank('Mains'), isNull);
    expect(eqIgnoreAsciiCase('Desserts', 'dESSERTS'), isTrue);
    expect(eqIgnoreAsciiCase('Él', 'él'), isFalse); // non-ASCII is compared exactly
  });

  test('editor suggestions: household categories first, then standard ones, deduplicated', () {
    expect(categorySuggestions(['desserts', 'Mains', ' ']).take(4), [
      'desserts',
      'Mains',
      'Breakfast',
      'Appetizers & Snacks',
    ]);
    final all = categorySuggestions(['desserts']);
    expect(all.where((c) => c.toLowerCase() == 'desserts'), ['desserts']);
    expect(all.length, larderCategories.length);
  });
}
