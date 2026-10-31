import 'dart:convert';
import 'dart:io';

import 'package:code_assets/code_assets.dart';
import 'package:flutter_rust_bridge_hooks/flutter_rust_bridge_hooks.dart';
import 'package:logging/logging.dart';

void main(List<String> args) async {
  await build(args, (input, output) async {
    if (!input.config.buildCodeAssets) return;
    final development = input.userDefines['development_http'];
    if (development != null && development is! bool) {
      throw const FormatException('development_http must be a boolean');
    }
    final policyInput = input.userDefines['app_update_policy'];
    final String appUpdatePolicy;
    if (policyInput == null || policyInput == '') {
      appUpdatePolicy = '';
    } else {
      if (policyInput is! String || utf8.encode(policyInput).length > 65536) {
        throw const FormatException(
          'app_update_policy must be a bounded JSON object string',
        );
      }
      Object? policy;
      try {
        policy = jsonDecode(policyInput);
      } on FormatException {
        // Do not include the policy body in compiler or hook diagnostics.
        throw const FormatException('app_update_policy must be a JSON object');
      }
      if (policy is! Map<String, dynamic>) {
        throw const FormatException('app_update_policy must be a JSON object');
      }
      appUpdatePolicy = policyInput;
    }
    final sourceCommit = input.userDefines['source_commit'];
    if (sourceCommit != null &&
        (sourceCommit is! String ||
            !RegExp(r'^[a-f0-9]{40}$').hasMatch(sourceCommit))) {
      throw const FormatException('source_commit must be an immutable Git SHA');
    }
    final workerDigest = input.userDefines['github_worker_sha256'];
    if (workerDigest != null &&
        (workerDigest is! String ||
            !RegExp(r'^[a-f0-9]{64}$').hasMatch(workerDigest))) {
      throw const FormatException(
        'github_worker_sha256 must be an artifact digest',
      );
    }
    final preparation = input.packageRoot.resolve('tool/prepare_native.py');
    final pythonExecutable = input.userDefines['native_build_python'];
    if (pythonExecutable != null &&
        (pythonExecutable is! String || !File(pythonExecutable).isAbsolute)) {
      throw const FormatException(
        'native_build_python must be an absolute path',
      );
    }
    final requestedCache = input.userDefines['native_build_cache'];
    if (requestedCache != null &&
        (requestedCache is! String || !Directory(requestedCache).isAbsolute)) {
      throw const FormatException(
        'native_build_cache must be an absolute path',
      );
    }
    final buildDirectory = input.userDefines['native_build_directory'];
    if (buildDirectory != null &&
        (buildDirectory is! String ||
            !Directory(buildDirectory).isAbsolute ||
            buildDirectory.length > 32768 ||
            utf8.encode(buildDirectory).length > 32768 ||
            utf8.decode(utf8.encode(buildDirectory)) != buildDirectory ||
            RegExp(r'[\x00-\x1f\x7f-\x9f]').hasMatch(buildDirectory))) {
      throw const FormatException(
        'native_build_directory must be an absolute path without controls of at most 32768 UTF-8 bytes',
      );
    }
    final cache =
        requestedCache as String? ??
        Directory.fromUri(input.outputDirectoryShared.resolve('nds-native/'))
            .path;
    final prepared = await Process.run(
      pythonExecutable as String? ??
          (Platform.isWindows ? 'python' : 'python3'),
      [
        File.fromUri(preparation).path,
        '--cache',
        cache,
        '--json',
        if (requestedCache != null) '--configured',
      ],
    );
    stderr.write(prepared.stderr);
    if (prepared.exitCode != 0) {
      throw const ProcessException(
        'prepare_native',
        [],
        'Native input preparation failed',
      );
    }
    final native = (jsonDecode(
      prepared.stdout as String,
    ) as Map<String, dynamic>).cast<String, String>();
    output.dependencies.addAll([
      preparation,
      input.packageRoot.resolve('rust/Cargo.toml'),
      input.packageRoot.resolve('rust/Cargo.lock'),
      input.packageRoot.resolve('rust/rust-toolchain.toml'),
      input.packageRoot.resolve('../../standarts.lock'),
      File('${native['cargo_home']}${Platform.pathSeparator}config.toml').uri,
    ]);
    // The upstream build helper logs its environment at CONFIG level. Never
    // forward those diagnostics; compiler failures still propagate as errors.
    final safeLogger = Logger.detached('nds_native_build')..level = Level.OFF;
    await FlutterRustBridgeNativeAssetsBuilder(
      cratePath: 'rust',
      buildMode: development == true
          ? FlutterRustBridgeBuildMode.debug
          : FlutterRustBridgeBuildMode.release,
      features: development == true ? const ['development-http'] : const [],
      extraCargoBuildArgs: const ['--locked', '--jobs', '4'],
      extraCargoEnvironmentVariables: {
        'CARGO_HOME': native['cargo_home']!,
        // The runner strips inherited CARGO_* variables. Only the tracked
        // input selects shared intermediates; final staging stays hook-owned.
        if (buildDirectory != null)
          'CARGO_BUILD_BUILD_DIR': buildDirectory as String,
        'CARGO_NET_GIT_FETCH_WITH_CLI': 'true',
        'GIT_CONFIG_GLOBAL': Platform.isWindows ? 'NUL' : '/dev/null',
        'GIT_CONFIG_NOSYSTEM': '1',
        'GIT_TERMINAL_PROMPT': '0',
        'NDS_NATIVE_BUILD_HOME': native['cache']!,
        'RUSTUP_AUTO_INSTALL': '0',
        'NDS_SOURCE_COMMIT': sourceCommit as String? ?? 'unknown',
        // This producer's edition comes from its owning, tracked source lock.
        'NDS_STANDARDS_RELEASE': native['standards_release']!,
        // userDefines participates in the Native Assets input identity. Always
        // replace any ambient policy, including when the assembly disables it.
        'NDS_APP_UPDATE_POLICY': appUpdatePolicy,
        if (workerDigest != null)
          'NDS_GITHUB_WORKER_SHA256': workerDigest as String,
      },
    ).run(input: input, output: output, logger: safeLogger);
  });
}
