import 'package:flutter_test/flutter_test.dart';
import 'package:larder/util/fractions.dart';

void main() {
  group('formatFraction', () {
    test('whole numbers', () {
      expect(formatFraction(2), '2');
      expect(formatFraction(0), '0');
    });
    test('common fractions', () {
      expect(formatFraction(1.5), '1 ½');
      expect(formatFraction(0.5), '½');
      expect(formatFraction(0.25), '¼');
      expect(formatFraction(0.75), '¾');
      expect(formatFraction(1 / 3), '⅓');
      expect(formatFraction(2 + 2 / 3), '2 ⅔');
      expect(formatFraction(0.125), '⅛');
    });
    test('non-fraction values fall back to decimals unless snapping', () {
      expect(formatFraction(1.43), '1.43');
      expect(formatFraction(1.43, snap: true), '1 ⅜');
      expect(formatFraction(0.97, snap: true), '1');
    });
    test('tiny values do not snap to zero', () {
      expect(formatFraction(0.03, snap: true), '0.03');
    });
    test('large values are rounded', () {
      expect(formatFraction(250.4), '250');
    });
  });

  group('parseQuantity', () {
    test('parses mixed forms', () {
      expect(parseQuantity('1 1/2'), 1.5);
      expect(parseQuantity('1½'), 1.5);
      expect(parseQuantity('3/4'), 0.75);
      expect(parseQuantity('2,5'), 2.5);
      expect(parseQuantity(''), isNull);
      expect(parseQuantity('abc'), isNull);
    });
  });
}
