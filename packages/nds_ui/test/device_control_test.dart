import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nds_native/nds_native.dart' as native;
import 'package:nds_ui/device_control.dart';
import 'package:nds_ui/l10n/app_localizations.dart';

class _Client implements native.NdsClient {
  _Client(this.current);
  native.DeviceControlView current;
  final writes = <(BigInt, String, List<String>)>[];

  @override
  Future<native.DeviceControlView> deviceControl() async => current;

  @override
  Future<native.DeviceControlView> setDeviceControl({
    required BigInt expectedRevision,
    required String mode,
    required List<String> purposes,
  }) async {
    writes.add((expectedRevision, mode, purposes));
    return current = native.DeviceControlView(
      mode: mode,
      purposes: purposes,
      revision: expectedRevision + BigInt.one,
      source: 'server',
    );
  }

  @override
  dynamic noSuchMethod(Invocation invocation) =>
      throw TestFailure('Unexpected client call: ${invocation.memberName}');
}

Future<void> _mount(
  WidgetTester tester,
  _Client client,
  Future<void> Function() changed,
) => tester.pumpWidget(
  MaterialApp(
    locale: const Locale('en'),
    localizationsDelegates: AppLocalizations.localizationsDelegates,
    supportedLocales: AppLocalizations.supportedLocales,
    home: Scaffold(
      body: SingleChildScrollView(
        child: DeviceControlCard(client: client, onChanged: changed),
      ),
    ),
  ),
);

void main() {
  testWidgets('saves a compare-and-set draft from the observed revision', (
    tester,
  ) async {
    final client = _Client(
      native.DeviceControlView(
        mode: 'fullauto',
        purposes: const ['desktop'],
        revision: BigInt.from(3),
        source: 'server',
      ),
    );
    var changed = 0;
    await _mount(tester, client, () async => changed++);
    await tester.pumpAndSettle();
    expect(find.text('Server policy, revision 3'), findsOneWidget);
    final save = find.byKey(const Key('device-control-save'));
    expect(tester.widget<FilledButton>(save).onPressed, isNull);
    await tester.tap(find.text('Manual'));
    await tester.tap(find.byKey(const Key('device-purpose-dev-desktop')));
    await tester.pumpAndSettle();
    await tester.tap(save);
    await tester.pumpAndSettle();
    expect(client.writes, [
      (BigInt.from(3), 'manual', ['desktop', 'dev-desktop']),
    ]);
    expect(changed, 1);
    expect(find.text('Server policy, revision 4'), findsOneWidget);
  });

  testWidgets('an unverified default keeps manual and offers no change', (
    tester,
  ) async {
    final client = _Client(
      native.DeviceControlView(
        mode: 'manual',
        purposes: const [],
        revision: BigInt.zero,
        source: 'default',
      ),
    );
    await _mount(tester, client, () async {});
    await tester.pumpAndSettle();
    expect(
      find.text('No server policy observed; manual settings apply.'),
      findsOneWidget,
    );
    await tester.tap(find.text('Full auto'));
    await tester.tap(find.byKey(const Key('device-purpose-dev-desktop')));
    await tester.pumpAndSettle();
    expect(
      tester
          .widget<FilledButton>(find.byKey(const Key('device-control-save')))
          .onPressed,
      isNull,
    );
    expect(client.writes, isEmpty);
  });
}
