import 'package:sembast_web/sembast_web.dart';

/// Opens the app database in IndexedDB (web).
Future<Database> openAppDatabase() => databaseFactoryWeb.openDatabase('larder');
