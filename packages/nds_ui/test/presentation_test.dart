import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/material.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nds_native/nds_native.dart' as native;
import 'package:nds_ui/nds_ui.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(() async {
    // Actual native composition; no identity/provider fake or network fixture.
    await native.RustLib.init();
    final loader = FontLoader(
      'packages/nddev_opennetwork_design_system/IBM Plex Sans',
    );
    for (final weight in ['Regular', 'Medium', 'SemiBold']) {
      loader.addFont(
        rootBundle.load(
          'packages/nddev_opennetwork_design_system/assets/fonts/IBMPlexSans-$weight.ttf',
        ),
      );
    }
    await loader.load();
    final icons = FontLoader('MaterialIcons')
      ..addFont(rootBundle.load('fonts/MaterialIcons-Regular.otf'));
    await icons.load();
  });
  for (final locale in ['en', 'ru']) {
    for (final compact in [false, true]) {
      testWidgets(
        'real NDS screens: $locale, ${compact ? '320px at 200%' : '1024px'}',
        (tester) async {
          tester.view.physicalSize = Size(compact ? 320 : 1024, 900);
          tester.view.devicePixelRatio = 1;
          tester.binding.platformDispatcher.localesTestValue = [Locale(locale)];
          tester.binding.platformDispatcher.textScaleFactorTestValue = compact
              ? 2
              : 1;
          addTearDown(tester.view.resetPhysicalSize);
          addTearDown(tester.view.resetDevicePixelRatio);
          addTearDown(tester.binding.platformDispatcher.clearLocalesTestValue);
          addTearDown(
            tester.binding.platformDispatcher.clearTextScaleFactorTestValue,
          );
          final key = GlobalKey();
          await tester.pumpWidget(
            RepaintBoundary(key: key, child: const NdsApp()),
          );
          // The anonymous shell must not discover private module adapters.
          // Positive enrolled-module interaction uses the owned backend fixture.
          expect(
            find.byKey(const Key('module-selector'), skipOffstage: false),
            findsNothing,
          );
          await tester.pumpAndSettle();
          expect(tester.takeException(), isNull);
          expect(find.byKey(const Key('connect')), findsOneWidget);
          expect(
            find.text(locale == 'ru' ? 'Подключение к NDS' : 'Connect to NDS'),
            findsOneWidget,
          );
          expect(
            MediaQuery.textScalerOf(
              tester.element(find.byKey(const Key('connect'))),
            ).scale(10),
            compact ? 20 : 10,
          );
          expect(find.byKey(const Key('server-address')), findsOneWidget);
          final theme = Theme.of(
            tester.element(find.byKey(const Key('connect'))),
          );
          expect(theme.colorScheme.primary, const Color(0xFFFFD83F));
          expect(
            theme.filledButtonTheme.style!.backgroundColor!.resolve({}),
            const Color(0xFFB8940E),
          );
          await capture(
            tester,
            key,
            '$locale-${compact ? 'compact' : 'desktop'}-connect',
          );
          await tester.tap(find.byIcon(Icons.devices));
          await tester.pumpAndSettle();
          expect(tester.takeException(), isNull);
          expect(find.byKey(const Key('module-selector')), findsNothing);
          expect(
            find.text(
              locale == 'ru'
                  ? 'Войдите и зарегистрируйте это устройство для управления модулями.'
                  : 'Sign in and register this device to manage modules.',
            ),
            findsOneWidget,
          );
          await capture(
            tester,
            key,
            '$locale-${compact ? 'compact' : 'desktop'}-modules',
          );
          await tester.pumpWidget(const SizedBox());
          await tester.pump();
        },
      );
    }
  }
}

Future<void> capture(WidgetTester tester, GlobalKey key, String name) async {
  final directory = Platform.environment['NDS_REVIEW_SHOTS'];
  if (directory == null) return;
  await tester.runAsync(() async {
    final boundary =
        key.currentContext!.findRenderObject()! as RenderRepaintBoundary;
    final image = await boundary.toImage();
    final bytes = await image.toByteData(format: ui.ImageByteFormat.png);
    await File('$directory/nds-$name.png')
        .writeAsBytes(bytes!.buffer.asUint8List());
    image.dispose();
  });
}
