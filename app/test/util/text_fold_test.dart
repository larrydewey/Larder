import 'package:flutter_test/flutter_test.dart';
import 'package:larder/util/text_fold.dart';

void main() {
  test('foldText matches Rust larder_core::text::fold', () {
    // Expected values produced by the Rust implementation.
    const cases = {
      'Crème  Brûlée': 'creme brulee',
      '  ÆBLESKIVER\tœuf ': 'aebleskiver oeuf',
      'Straße': 'strasse',
      'Łódź Żurek': 'lodz zurek',
      'ÀÁÂÃÄÅĀ ÇČĆ ÈÉÊËĒĘ': 'aaaaaaa ccc eeeeee',
      'ÌÍÎÏĪ ÑŃ ÒÓÔÕÖØŌ ÙÚÛÜŪ ÝŸ ŚŠ ŹŻŽ': 'iiiii nn ooooooo uuuuu yy ss zzz',
      // Not in Rust's table: only lower-cased.
      'Ăă Őő Ţ': 'ăă őő ţ',
      // Rune-by-rune lower-casing: no final-sigma rule.
      'ΣΟΦΟΣ': 'σοφοσ',
      'a b  c\n': 'a b c',
      '': '',
    };
    cases.forEach((input, want) => expect(foldText(input), want, reason: input));
  });

  test('collapseWs', () {
    expect(collapseWs('  a \t b\n\nc  '), 'a b c');
    expect(collapseWs('　'), '');
  });

  test('DuplicateIndex follows the server find_duplicate rule', () {
    final idx = DuplicateIndex()
      ..add('1', name: 'Crème Brûlée')
      ..add('2', name: 'Soup', sourceUrl: 'https://ex.com/soup')
      ..add('3', name: 'soup');
    expect(idx.find(name: 'CREME   brulee'), '1');
    expect(idx.find(name: 'Different', sourceUrl: 'https://ex.com/soup'), '2');
    // URL wins over name, like the server's two-step lookup.
    expect(idx.find(name: 'creme brulee', sourceUrl: 'https://ex.com/soup'), '2');
    expect(idx.find(name: 'SOUP'), '2');
    expect(idx.find(name: 'Cremes Brulees'), isNull);
    // URLs are compared exactly.
    expect(idx.find(name: 'x', sourceUrl: 'https://ex.com/SOUP'), isNull);
  });
}
