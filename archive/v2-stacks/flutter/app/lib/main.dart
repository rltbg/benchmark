import 'package:flutter/widgets.dart';

import 'bench.dart';
import 'scenarios.dart';

void main(List<String> argv) {
  WidgetsFlutterBinding.ensureInitialized();
  final cfg = Cfg.parse(argv);
  Bench.I = Bench(cfg);
  final Widget home;
  switch (cfg.scenario) {
    case 'text':
      home = TextScenario(cfg);
    case 'images':
      home = ImagesScenario(cfg);
    case 'nav':
      home = NavScenario(cfg);
    default:
      home = Host(cfg);
  }
  runApp(
    WidgetsApp(
      color: const Color(0xFF0D1117),
      debugShowCheckedModeBanner: false,
      navigatorKey: navKey,
      pageRouteBuilder: <T>(RouteSettings s, WidgetBuilder b) =>
          PageRouteBuilder<T>(settings: s, pageBuilder: (c, a, sa) => b(c)),
      builder: (c, child) => DefaultTextStyle(
        style: const TextStyle(color: Color(0xFFE6EDF3), fontSize: 14, decoration: TextDecoration.none),
        child: child!,
      ),
      home: home,
    ),
  );
  Bench.I.begin();
}
