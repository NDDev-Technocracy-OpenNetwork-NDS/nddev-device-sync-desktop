// Presentation ordering only. Actual ScreenViewer decodes a synthetic pixel;
// no Rust authority, remote screen, native capture or physical drain is mocked
// into a successful acceptance claim.
import 'dart:async';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:nds_native/nds_native.dart' as native;
import 'package:nds_ui/activity.dart';
import 'package:nds_ui/l10n/app_localizations.dart';
import 'package:nds_ui/screen_viewer.dart';

void main() {
  for (final outcome in ['current', 'refused', 'changed after validation']) {
    testWidgets('renderer awaits original frame validation: $outcome', (
      tester,
    ) async {
      final fixture = _Fixture();
      try {
        await tester.pumpWidget(
          ApplicationActivityScope(
            activity: fixture.activity,
            child: MaterialApp(
              localizationsDelegates: AppLocalizations.localizationsDelegates,
              supportedLocales: AppLocalizations.supportedLocales,
              home: ScreenViewer(
                client: fixture.client,
                deviceId: 'synthetic-device',
                organization: fixture.organization,
              ),
            ),
          ),
        );
        await _until(tester, () => fixture.frame.validations == 1);
        expect(fixture.frame.reads, 1);
        expect(fixture.client.personalCalls, 0);
        expect(fixture.client.organizationCalls, 1);
        expect(fixture.frame.liveChecks, 0);
        expect(find.byType(RawImage), findsNothing);
        if (outcome == 'changed after validation') fixture.frame.live = false;
        fixture.frame.validation.complete(outcome != 'refused');
        if (outcome == 'current') {
          await _until(
            tester,
            () => find.byType(RawImage).evaluate().isNotEmpty,
          );
          expect(fixture.frame.liveChecks, 1);
        } else {
          await _until(tester, () => fixture.view.closes > 0);
          expect(find.byType(RawImage), findsNothing);
          expect(fixture.frame.liveChecks, outcome == 'refused' ? 0 : 1);
        }
      } finally {
        // Settle every declared gate even when a presentation assertion fails.
        fixture.release();
        try {
          await tester.pumpWidget(const SizedBox.shrink());
          for (var i = 0; i < 16; i++) {
            await tester.pump(const Duration(milliseconds: 1));
          }
        } finally {
          fixture.activity.dispose();
        }
        expect(fixture.frame.validation.isCompleted, isTrue);
        expect(fixture.view.stopped.isCompleted, isTrue);
        expect(fixture.view.nextFrame.isCompleted, isTrue);
      }
      expect(fixture.unexpected, isEmpty);
      expect(fixture.frame.disposals, 1);
    }, timeout: const Timeout(Duration(seconds: 30)));
  }
}

Future<void> _until(WidgetTester tester, bool Function() done) async {
  for (var i = 0; i < 128; i++) {
    await tester.pump(const Duration(milliseconds: 1));
    // Image decoding completes outside Flutter's fake timer clock.
    await tester.runAsync(
      () => Future<void>.delayed(const Duration(milliseconds: 10)),
    );
    if (done()) return;
  }
  fail('screen presentation did not reach its finite fixture boundary');
}

class _Fixture {
  _Fixture() {
    frame = _Frame(unexpected);
    view = _View(frame, unexpected);
    organization = _Organization(unexpected);
    client = _Client(view, organization, unexpected);
    activity.value = true;
  }
  final unexpected = <String>[];
  final activity = ApplicationActivity(nativeReady: false);
  late final _Frame frame;
  late final _View view;
  late final _Client client;
  late final _Organization organization;
  void release() {
    if (!frame.validation.isCompleted) frame.validation.complete(false);
    view.release();
  }
}

class _Client implements native.NdsClient {
  _Client(this.view, this.organization, this.unexpected);
  final _View view;
  final _Organization organization;
  final List<String> unexpected;
  var personalCalls = 0;
  var organizationCalls = 0;
  @override
  Future<native.ScreenView> screenOpen({required String target}) async {
    personalCalls++;
    return view;
  }

  @override
  Future<native.ScreenView> screenOpenForOrganization({
    required native.LocalOrganization organization,
    required String target,
  }) async {
    organizationCalls++;
    if (!identical(organization, this.organization)) {
      throw TestFailure('viewer replaced the original organizational context');
    }
    return view;
  }

  @override
  void screenCancel() {}
  @override
  Future<void> screenStop() async => view.release();
  @override
  dynamic noSuchMethod(Invocation invocation) {
    unexpected.add(invocation.memberName.toString());
    throw TestFailure('unexpected client method: ${invocation.memberName}');
  }
}

class _Organization implements native.LocalOrganization {
  _Organization(this.unexpected);
  final List<String> unexpected;
  @override
  bool isCurrent() => true;
  @override
  dynamic noSuchMethod(Invocation invocation) {
    unexpected.add(invocation.memberName.toString());
    throw TestFailure(
      'unexpected organization method: ${invocation.memberName}',
    );
  }
}

class _View implements native.ScreenView {
  _View(this.value, this.unexpected);
  final _Frame value;
  final List<String> unexpected;
  final stopped = Completer<void>();
  final nextFrame = Completer<void>();
  var frames = 0;
  var closes = 0;
  @override
  Future<native.ScreenFrame> frame() async {
    if (frames++ == 0) return value;
    await nextFrame.future;
    throw StateError('synthetic viewer closed');
  }

  @override
  Future<void> closed() => stopped.future;
  @override
  Future<void> close() async {
    closes++;
    release();
  }

  void release() {
    if (!stopped.isCompleted) stopped.complete();
    if (!nextFrame.isCompleted) nextFrame.complete();
  }

  @override
  void dispose() {}
  @override
  dynamic noSuchMethod(Invocation invocation) {
    unexpected.add(invocation.memberName.toString());
    throw TestFailure('unexpected viewer method: ${invocation.memberName}');
  }
}

class _Frame implements native.ScreenFrame {
  _Frame(this.unexpected);
  final List<String> unexpected;
  final validation = Completer<bool>();
  var validations = 0;
  var liveChecks = 0;
  var reads = 0;
  var disposals = 0;
  var live = true;
  @override
  native.ScreenLayout layout() =>
      const native.ScreenLayout(width: 1, height: 1, stride: 4);
  @override
  Future<Uint8List> readOnce() async {
    reads++;
    return Uint8List.fromList([0, 0, 255, 255]);
  }

  @override
  Future<void> validateContext() async {
    validations++;
    if (!await validation.future) throw StateError('original context refused');
  }

  @override
  void checkLive() {
    liveChecks++;
    if (!live) throw StateError('original context changed');
  }

  @override
  void dispose() => disposals++;
  @override
  dynamic noSuchMethod(Invocation invocation) {
    unexpected.add(invocation.memberName.toString());
    throw TestFailure('unexpected frame method: ${invocation.memberName}');
  }
}
