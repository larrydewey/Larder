import 'dart:convert';
import 'dart:io';

import 'package:http/http.dart' as http;
import 'package:http/testing.dart';
import 'package:larder/engine/engine.dart';
import 'package:larder/models/json.dart';
import 'package:sembast/sembast_memory.dart';
import 'package:larder/data/local_store.dart';

int _dbCounter = 0;

Future<LocalStore> memoryStore() async => LocalStore(
  await databaseFactoryMemory.openDatabase('test-${_dbCounter++}-${DateTime.now().microsecondsSinceEpoch}'),
);

/// A recorded HTTP request.
class Req {
  Req(this.method, this.path, this.body, this.headers);
  final String method;
  final String path;
  final Object? body;
  final Map<String, String> headers;

  @override
  String toString() => '$method $path';
}

typedef Handler = http.Response? Function(Req req);

/// Fake server: tries [handlers] in order; unmatched requests get 404.
class FakeServer {
  final List<Req> requests = [];
  final List<Handler> handlers = [];
  bool offline = false;

  void on(String method, Pattern path, http.Response Function(Req) respond) {
    handlers.add((r) {
      if (r.method != method) return null;
      final ok = path is RegExp ? path.hasMatch(r.path) : r.path == path;
      return ok ? respond(r) : null;
    });
  }

  late final client = MockClient((request) async {
    if (offline) throw http.ClientException('offline');
    Object? body;
    if (request.headers['content-type']?.contains('json') ?? false) {
      body = request.body.isEmpty ? null : jsonDecode(request.body);
    } else if (request.bodyBytes.isNotEmpty) {
      body = request.bodyBytes;
    }
    final r = Req(request.method, request.url.path, body, request.headers);
    requests.add(r);
    for (final h in handlers) {
      final res = h(r);
      if (res != null) return res;
    }
    return json({'error': 'no route', 'code': 'not_found'}, 404);
  });

  List<String> get log => requests.map((r) => r.toString()).toList();
}

http.Response json(Object? body, [int status = 200]) =>
    http.Response(jsonEncode(body), status, headers: {'content-type': 'application/json'});

/// Engine fake: implements the methods the app uses with simple logic.
class FakeEngine implements Engine {
  final List<String> calls = [];
  final Map<String, Object? Function(Json args)> overrides = {};

  @override
  Future<Object?> call(String method, Map<String, Object?> args) async {
    calls.add(method);
    final o = overrides[method];
    if (o != null) return o(asJson(args));
    switch (method) {
      case 'version':
        return {'version': 'fake'};
      case 'normalize_recipe':
        final j = Map<String, Object?>.from(args);
        j['name'] = asStr(j['name']).trim();
        j['ingredients'] = [
          for (final i in asJsonList(j['ingredients']))
            i.containsKey('food') ? i : {..._parseLine(asStr(i['original'])), 'section': i['section']},
        ];
        j['slug'] = asStr(j['name']).toLowerCase().replaceAll(' ', '-');
        j['search_text'] = asStr(j['name']).toLowerCase();
        return j;
      case 'parse_shopping_item':
        final p = _parseLine(asStr(args['text']));
        return {...p, 'aisle': 'Produce', 'display': asStr(args['text'])};
      case 'merge_shopping':
        final items = asJsonList(args['items']).toList();
        var n = 0;
        for (final a in asJsonList(args['add'])) {
          final recipe = asJson(a['recipe']);
          for (final i in asJsonList(recipe['ingredients'])) {
            items.add({
              'id': 'merged-${n++}',
              'food': i['food'],
              'quantity': i['quantity'],
              'unit': i['unit'],
              'display': i['original'],
              'aisle': 'Other',
              'checked': false,
              'position': items.length,
              'recipe_ids': [recipe['id']],
            });
          }
        }
        return {'items': items};
      case 'process_image':
        final out = <String, String>{};
        for (final size in ['thumb', 'medium', 'full']) {
          final path = '${args['output_dir']}/${args['id']}-$size.jpg';
          File(asStr(args['input_path'])).copySync(path);
          out[size] = path;
        }
        return out;
      case 'cookbook_themes':
        return [
          {'id': 'classic', 'name': 'Classic', 'description': 'Serif'},
        ];
      case 'render_cookbook':
      case 'export_backup':
        File(asStr(args['out_path'])).writeAsBytesSync(utf8.encode('$method:${jsonEncode(args['recipes'])}'));
        return {'path': args['out_path']};
      case 'suggest_categories':
        return {
          'categories': ['Breakfast', 'Soups & Stews', 'Main Dishes', 'Desserts'],
          'suggestions': [
            for (final r in asJsonList(args['recipes'])) {'id': r['id'], 'category': fakeCategory(asStr(r['name']))},
          ],
        };
      case 'import_text':
        return {'name': 'From text', 'ingredients': [], 'instructions': []};
    }
    throw EngineException('unknown method $method', code: 'bad_request');
  }

  /// The fake engine's category rule (by recipe name).
  static String? fakeCategory(String name) {
    final n = name.toLowerCase();
    if (n.contains('pancake')) return 'Breakfast';
    if (n.contains('soup')) return 'Soups & Stews';
    if (n.contains('cake') || n.contains('pie')) return 'Desserts';
    if (n.contains('stew') || n.contains('roast')) return 'Main Dishes';
    return null;
  }

  static Json _parseLine(String line) {
    final m = RegExp(r'^(\d+(?:\.\d+)?)\s+(\w+)\s+(.*)$').firstMatch(line.trim());
    if (m == null) return {'original': line, 'food': line, 'quantity': null, 'unit': null};
    return {'original': line, 'quantity': double.parse(m.group(1)!), 'unit': m.group(2), 'food': m.group(3)};
  }
}
