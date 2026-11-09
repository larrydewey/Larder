import 'package:flutter_test/flutter_test.dart';
import 'package:larder/models/recipe.dart';
import 'package:larder/util/share.dart';

Recipe _recipe() => Recipe.fromJson({
  'id': 'abc-123',
  'household_id': 'h1',
  'slug': 'tomato-soup',
  'name': 'Tomato Soup',
  'description': 'Our version, with less salt.',
  'source_url': 'https://example.com/original-tomato-soup',
  'source_name': 'example.com',
  'notes': 'Freezes well.',
  'ingredients': [
    {'original': '2 lb tomatoes'},
    {'original': '1 onion'},
  ],
  'instructions': [
    {'text': 'Roast the tomatoes.'},
    {'text': 'Blend with the onion.'},
  ],
});

void main() {
  test('connected: shares the name and the Larder link, not the source', () {
    final r = _recipe();
    final link = recipeLink('https://recipes.example.org', r);
    expect(link.toString(), 'https://recipes.example.org/recipes/abc-123');
    final text = recipeShareText(r, link: link);
    expect(text, 'Tomato Soup\n\nhttps://recipes.example.org/recipes/abc-123');
    expect(text, isNot(contains('example.com')));
  });

  test('link keeps a port and a sub-path, and drops a trailing slash', () {
    final r = _recipe();
    expect(recipeLink('http://192.168.1.10:8080/', r).toString(), 'http://192.168.1.10:8080/recipes/abc-123');
    expect(recipeLink('https://host.example/larder', r).toString(), 'https://host.example/larder/recipes/abc-123');
  });

  test('no usable server address gives no link', () {
    expect(recipeLink('', _recipe()), isNull);
    expect(recipeLink('not a url', _recipe()), isNull);
  });

  test('standalone: shares the saved recipe as text, without the source', () {
    final text = recipeShareText(_recipe());
    expect(
      text,
      'Tomato Soup\n\n'
      'Our version, with less salt.\n\n'
      'Ingredients\n- 2 lb tomatoes\n- 1 onion\n\n'
      'Instructions\n1. Roast the tomatoes.\n2. Blend with the onion.\n\n'
      'Notes\nFreezes well.',
    );
    expect(text, isNot(contains('example.com')));
  });
}
