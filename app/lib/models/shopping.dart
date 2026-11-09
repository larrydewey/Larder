import 'json.dart';

/// Fixed aisle ordering from the API.
const aisles = [
  'Produce',
  'Meat & Seafood',
  'Dairy & Eggs',
  'Bakery',
  'Pantry',
  'Baking',
  'Spices & Seasonings',
  'Canned & Jarred',
  'Frozen',
  'Beverages',
  'Household',
  'Other',
];

int aisleOrder(String? aisle) {
  final i = aisles.indexOf(aisle ?? 'Other');
  return i < 0 ? aisles.length : i;
}

class ShoppingItem {
  const ShoppingItem({
    required this.id,
    required this.listId,
    this.food,
    this.quantity,
    this.unit,
    this.note,
    this.display = '',
    this.aisle,
    this.checked = false,
    this.position = 0,
    this.recipeIds = const [],
    this.updatedAt,
  });

  final String id;
  final String listId;
  final String? food;
  final double? quantity;
  final String? unit;
  final String? note;
  final String display;
  final String? aisle;
  final bool checked;
  final int position;
  final List<String> recipeIds;
  final DateTime? updatedAt;

  String get label => display.isNotEmpty ? display : (food ?? '');

  factory ShoppingItem.fromJson(Json j) => ShoppingItem(
    id: asStr(j['id']),
    listId: asStr(j['list_id']),
    food: asString(j['food']),
    quantity: asDouble(j['quantity']),
    unit: asString(j['unit']),
    note: asString(j['note']),
    display: asStr(j['display']),
    aisle: asString(j['aisle']),
    checked: asBool(j['checked']),
    position: asInt(j['position']) ?? 0,
    recipeIds: asStringList(j['recipe_ids']),
    updatedAt: asDate(j['updated_at']),
  );

  Json toJson() => {
    'id': id,
    'list_id': listId,
    'food': food,
    'quantity': quantity,
    'unit': unit,
    'note': note,
    'display': display,
    'aisle': aisle,
    'checked': checked,
    'position': position,
    'recipe_ids': recipeIds,
    'updated_at': updatedAt?.toUtc().toIso8601String(),
  };

  ShoppingItem copyWith({
    String? food,
    double? Function()? quantity,
    String? Function()? unit,
    String? Function()? note,
    String? display,
    String? aisle,
    bool? checked,
    int? position,
  }) => ShoppingItem(
    id: id,
    listId: listId,
    food: food ?? this.food,
    quantity: quantity != null ? quantity() : this.quantity,
    unit: unit != null ? unit() : this.unit,
    note: note != null ? note() : this.note,
    display: display ?? this.display,
    aisle: aisle ?? this.aisle,
    checked: checked ?? this.checked,
    position: position ?? this.position,
    recipeIds: recipeIds,
    updatedAt: DateTime.now().toUtc(),
  );
}

class ShoppingList {
  const ShoppingList({
    required this.id,
    required this.name,
    this.createdAt,
    this.updatedAt,
    this.itemCount = 0,
    this.checkedCount = 0,
    this.items,
  });

  final String id;
  final String name;
  final DateTime? createdAt;
  final DateTime? updatedAt;
  final int itemCount;
  final int checkedCount;

  /// Null when the payload did not include items.
  final List<ShoppingItem>? items;

  factory ShoppingList.fromJson(Json j) => ShoppingList(
    id: asStr(j['id']),
    name: asStr(j['name']),
    createdAt: asDate(j['created_at']),
    updatedAt: asDate(j['updated_at']),
    itemCount: asInt(j['item_count']) ?? 0,
    checkedCount: asInt(j['checked_count']) ?? 0,
    items: j['items'] is List ? asJsonList(j['items']).map(ShoppingItem.fromJson).toList() : null,
  );

  Json toJson() => {
    'id': id,
    'name': name,
    'created_at': createdAt?.toUtc().toIso8601String(),
    'updated_at': updatedAt?.toUtc().toIso8601String(),
    'item_count': itemCount,
    'checked_count': checkedCount,
    if (items != null) 'items': items!.map((e) => e.toJson()).toList(),
  };

  /// Returns a copy with [items] and recomputed counts.
  ShoppingList withItems(List<ShoppingItem> newItems, {String? name}) => ShoppingList(
    id: id,
    name: name ?? this.name,
    createdAt: createdAt,
    updatedAt: DateTime.now().toUtc(),
    itemCount: newItems.length,
    checkedCount: newItems.where((e) => e.checked).length,
    items: newItems,
  );

  ShoppingList rename(String newName) => ShoppingList(
    id: id,
    name: newName,
    createdAt: createdAt,
    updatedAt: DateTime.now().toUtc(),
    itemCount: itemCount,
    checkedCount: checkedCount,
    items: items,
  );
}
