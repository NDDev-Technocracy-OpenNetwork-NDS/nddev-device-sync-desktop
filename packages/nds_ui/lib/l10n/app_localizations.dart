import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:intl/intl.dart' as intl;

import 'app_localizations_en.dart';
import 'app_localizations_ru.dart';

// ignore_for_file: type=lint

/// Callers can lookup localized strings with an instance of AppLocalizations
/// returned by `AppLocalizations.of(context)`.
///
/// Applications need to include `AppLocalizations.delegate()` in their app's
/// `localizationDelegates` list, and the locales they support in the app's
/// `supportedLocales` list. For example:
///
/// ```dart
/// import 'l10n/app_localizations.dart';
///
/// return MaterialApp(
///   localizationsDelegates: AppLocalizations.localizationsDelegates,
///   supportedLocales: AppLocalizations.supportedLocales,
///   home: MyApplicationHome(),
/// );
/// ```
///
/// ## Update pubspec.yaml
///
/// Please make sure to update your pubspec.yaml to include the following
/// packages:
///
/// ```yaml
/// dependencies:
///   # Internationalization support.
///   flutter_localizations:
///     sdk: flutter
///   intl: any # Use the pinned version from flutter_localizations
///
///   # Rest of dependencies
/// ```
///
/// ## iOS Applications
///
/// iOS applications define key application metadata, including supported
/// locales, in an Info.plist file that is built into the application bundle.
/// To configure the locales supported by your app, you’ll need to edit this
/// file.
///
/// First, open your project’s ios/Runner.xcworkspace Xcode workspace file.
/// Then, in the Project Navigator, open the Info.plist file under the Runner
/// project’s Runner folder.
///
/// Next, select the Information Property List item, select Add Item from the
/// Editor menu, then select Localizations from the pop-up menu.
///
/// Select and expand the newly-created Localizations item then, for each
/// locale your application supports, add a new item and select the locale
/// you wish to add from the pop-up menu in the Value field. This list should
/// be consistent with the languages listed in the AppLocalizations.supportedLocales
/// property.
abstract class AppLocalizations {
  AppLocalizations(String locale)
    : localeName = intl.Intl.canonicalizedLocale(locale.toString());

  final String localeName;

  static AppLocalizations? of(BuildContext context) {
    return Localizations.of<AppLocalizations>(context, AppLocalizations);
  }

  static const LocalizationsDelegate<AppLocalizations> delegate =
      _AppLocalizationsDelegate();

  /// A list of this localizations delegate along with the default localizations
  /// delegates.
  ///
  /// Returns a list of localizations delegates containing this delegate along with
  /// GlobalMaterialLocalizations.delegate, GlobalCupertinoLocalizations.delegate,
  /// and GlobalWidgetsLocalizations.delegate.
  ///
  /// Additional delegates can be added by appending to this list in
  /// MaterialApp. This list does not have to be used at all if a custom list
  /// of delegates is preferred or required.
  static const List<LocalizationsDelegate<dynamic>> localizationsDelegates =
      <LocalizationsDelegate<dynamic>>[
        delegate,
        GlobalMaterialLocalizations.delegate,
        GlobalCupertinoLocalizations.delegate,
        GlobalWidgetsLocalizations.delegate,
      ];

  /// A list of this localizations delegate's supported locales.
  static const List<Locale> supportedLocales = <Locale>[
    Locale('en'),
    Locale('ru'),
  ];

  /// No description provided for @appTitle.
  ///
  /// In en, this message translates to:
  /// **'NDDev OpenNetwork'**
  String get appTitle;

  /// No description provided for @connectTitle.
  ///
  /// In en, this message translates to:
  /// **'Connect to NDS'**
  String get connectTitle;

  /// No description provided for @connectHelp.
  ///
  /// In en, this message translates to:
  /// **'Enter the HTTPS address of your self-hosted NDS server.'**
  String get connectHelp;

  /// No description provided for @serverAddress.
  ///
  /// In en, this message translates to:
  /// **'Server address'**
  String get serverAddress;

  /// No description provided for @connect.
  ///
  /// In en, this message translates to:
  /// **'Connect'**
  String get connect;

  /// No description provided for @signIn.
  ///
  /// In en, this message translates to:
  /// **'Sign in'**
  String get signIn;

  /// No description provided for @email.
  ///
  /// In en, this message translates to:
  /// **'Email address'**
  String get email;

  /// No description provided for @sendCode.
  ///
  /// In en, this message translates to:
  /// **'Send sign-in code'**
  String get sendCode;

  /// No description provided for @emailHint.
  ///
  /// In en, this message translates to:
  /// **'If this address is permitted, a sign-in code will arrive.'**
  String get emailHint;

  /// No description provided for @code.
  ///
  /// In en, this message translates to:
  /// **'Eight-digit code'**
  String get code;

  /// No description provided for @verify.
  ///
  /// In en, this message translates to:
  /// **'Verify code'**
  String get verify;

  /// No description provided for @resend.
  ///
  /// In en, this message translates to:
  /// **'Send another code'**
  String get resend;

  /// No description provided for @github.
  ///
  /// In en, this message translates to:
  /// **'Sign in with GitHub'**
  String get github;

  /// No description provided for @githubHelp.
  ///
  /// In en, this message translates to:
  /// **'Compare this code with the NDS approval page in your browser. Approve only when both codes match.'**
  String get githubHelp;

  /// No description provided for @openGithub.
  ///
  /// In en, this message translates to:
  /// **'Open GitHub'**
  String get openGithub;

  /// No description provided for @awaitApproval.
  ///
  /// In en, this message translates to:
  /// **'Waiting for explicit browser approval'**
  String get awaitApproval;

  /// No description provided for @cancel.
  ///
  /// In en, this message translates to:
  /// **'Cancel sign-in'**
  String get cancel;

  /// No description provided for @unavailable.
  ///
  /// In en, this message translates to:
  /// **'Unavailable on this server'**
  String get unavailable;

  /// No description provided for @sessionTitle.
  ///
  /// In en, this message translates to:
  /// **'Your NDS session'**
  String get sessionTitle;

  /// No description provided for @user.
  ///
  /// In en, this message translates to:
  /// **'User'**
  String get user;

  /// No description provided for @instance.
  ///
  /// In en, this message translates to:
  /// **'Server'**
  String get instance;

  /// No description provided for @method.
  ///
  /// In en, this message translates to:
  /// **'Sign-in method'**
  String get method;

  /// No description provided for @expires.
  ///
  /// In en, this message translates to:
  /// **'Expires'**
  String get expires;

  /// No description provided for @verified.
  ///
  /// In en, this message translates to:
  /// **'Last verified'**
  String get verified;

  /// No description provided for @unverified.
  ///
  /// In en, this message translates to:
  /// **'This session has not been revalidated. Check the server connection.'**
  String get unverified;

  /// No description provided for @refresh.
  ///
  /// In en, this message translates to:
  /// **'Revalidate session'**
  String get refresh;

  /// No description provided for @logout.
  ///
  /// In en, this message translates to:
  /// **'Revoke session and sign out'**
  String get logout;

  /// No description provided for @changeServer.
  ///
  /// In en, this message translates to:
  /// **'Change server'**
  String get changeServer;

  /// No description provided for @diagnostics.
  ///
  /// In en, this message translates to:
  /// **'Diagnostic logs for five minutes'**
  String get diagnostics;

  /// No description provided for @diagnosticsOff.
  ///
  /// In en, this message translates to:
  /// **'Stop diagnostic logs'**
  String get diagnosticsOff;

  /// No description provided for @privacy.
  ///
  /// In en, this message translates to:
  /// **'Session credentials and vault keys stay in native secure storage. The server delivers your vault key after verified sign-in and device enrollment.'**
  String get privacy;

  /// No description provided for @endpointError.
  ///
  /// In en, this message translates to:
  /// **'Use an HTTPS server origin without credentials, path, query or fragment.'**
  String get endpointError;

  /// No description provided for @inputError.
  ///
  /// In en, this message translates to:
  /// **'Check the email address and the eight-digit code.'**
  String get inputError;

  /// No description provided for @networkError.
  ///
  /// In en, this message translates to:
  /// **'The server could not be reached. No fresh session state is confirmed.'**
  String get networkError;

  /// No description provided for @storeError.
  ///
  /// In en, this message translates to:
  /// **'Protected credential storage is unavailable. No plaintext fallback is used.'**
  String get storeError;

  /// No description provided for @authError.
  ///
  /// In en, this message translates to:
  /// **'Sign-in could not be confirmed. Check the code or start again.'**
  String get authError;

  /// No description provided for @expiredError.
  ///
  /// In en, this message translates to:
  /// **'This sign-in attempt expired. Start again.'**
  String get expiredError;

  /// No description provided for @rateError.
  ///
  /// In en, this message translates to:
  /// **'Please wait before trying again.'**
  String get rateError;

  /// No description provided for @cancelledError.
  ///
  /// In en, this message translates to:
  /// **'Sign-in cancelled.'**
  String get cancelledError;

  /// No description provided for @responseError.
  ///
  /// In en, this message translates to:
  /// **'The server response does not match the NDS contract.'**
  String get responseError;

  /// No description provided for @personalContextServerRequired.
  ///
  /// In en, this message translates to:
  /// **'Update the NDS server before using this version of the client.'**
  String get personalContextServerRequired;

  /// No description provided for @genericError.
  ///
  /// In en, this message translates to:
  /// **'The operation failed. Try again.'**
  String get genericError;

  /// No description provided for @launchError.
  ///
  /// In en, this message translates to:
  /// **'The authorization browser could not be opened.'**
  String get launchError;

  /// No description provided for @working.
  ///
  /// In en, this message translates to:
  /// **'Working…'**
  String get working;

  /// No description provided for @validFor.
  ///
  /// In en, this message translates to:
  /// **'{seconds, plural, one{Valid for {seconds} second} other{Valid for {seconds} seconds}}'**
  String validFor(int seconds);

  /// No description provided for @retryIn.
  ///
  /// In en, this message translates to:
  /// **'{seconds, plural, one{Retry in {seconds} second} other{Retry in {seconds} seconds}}'**
  String retryIn(int seconds);

  /// No description provided for @revalidationRequired.
  ///
  /// In en, this message translates to:
  /// **'The credential write may have completed. Revalidate the session before continuing.'**
  String get revalidationRequired;

  /// No description provided for @emailMethod.
  ///
  /// In en, this message translates to:
  /// **'Email code'**
  String get emailMethod;

  /// No description provided for @sessionTab.
  ///
  /// In en, this message translates to:
  /// **'NDS session'**
  String get sessionTab;

  /// No description provided for @localModules.
  ///
  /// In en, this message translates to:
  /// **'Local modules'**
  String get localModules;

  /// No description provided for @localReadOnly.
  ///
  /// In en, this message translates to:
  /// **'Inspect local tools, history and planned maintenance. Tools retain control of their data.'**
  String get localReadOnly;

  /// No description provided for @module.
  ///
  /// In en, this message translates to:
  /// **'Module'**
  String get module;

  /// No description provided for @systemInfo.
  ///
  /// In en, this message translates to:
  /// **'System information'**
  String get systemInfo;

  /// No description provided for @clipboardHistory.
  ///
  /// In en, this message translates to:
  /// **'Clipboard history'**
  String get clipboardHistory;

  /// No description provided for @cleaner.
  ///
  /// In en, this message translates to:
  /// **'Cleaner'**
  String get cleaner;

  /// No description provided for @updater.
  ///
  /// In en, this message translates to:
  /// **'Updates'**
  String get updater;

  /// No description provided for @providerUnavailable.
  ///
  /// In en, this message translates to:
  /// **'The local tool is unavailable.'**
  String get providerUnavailable;

  /// No description provided for @providerFailed.
  ///
  /// In en, this message translates to:
  /// **'The local tool could not complete this request.'**
  String get providerFailed;

  /// No description provided for @notChecked.
  ///
  /// In en, this message translates to:
  /// **'Not checked'**
  String get notChecked;

  /// No description provided for @reachable.
  ///
  /// In en, this message translates to:
  /// **'Available'**
  String get reachable;

  /// No description provided for @noRecords.
  ///
  /// In en, this message translates to:
  /// **'No records.'**
  String get noRecords;

  /// No description provided for @nextPage.
  ///
  /// In en, this message translates to:
  /// **'Next page'**
  String get nextPage;

  /// No description provided for @openText.
  ///
  /// In en, this message translates to:
  /// **'Read text'**
  String get openText;

  /// No description provided for @cpuUsage.
  ///
  /// In en, this message translates to:
  /// **'CPU usage'**
  String get cpuUsage;

  /// No description provided for @cpuTemperature.
  ///
  /// In en, this message translates to:
  /// **'CPU temperature'**
  String get cpuTemperature;

  /// No description provided for @memoryUsage.
  ///
  /// In en, this message translates to:
  /// **'Memory used'**
  String get memoryUsage;

  /// No description provided for @swapUsage.
  ///
  /// In en, this message translates to:
  /// **'Swap used'**
  String get swapUsage;

  /// No description provided for @gpuUsage.
  ///
  /// In en, this message translates to:
  /// **'GPU usage'**
  String get gpuUsage;

  /// No description provided for @gpuMemory.
  ///
  /// In en, this message translates to:
  /// **'GPU memory used'**
  String get gpuMemory;

  /// No description provided for @gpuTemperature.
  ///
  /// In en, this message translates to:
  /// **'GPU temperature'**
  String get gpuTemperature;

  /// No description provided for @diskRead.
  ///
  /// In en, this message translates to:
  /// **'Disk read rate'**
  String get diskRead;

  /// No description provided for @diskWrite.
  ///
  /// In en, this message translates to:
  /// **'Disk write rate'**
  String get diskWrite;

  /// No description provided for @diskTemperature.
  ///
  /// In en, this message translates to:
  /// **'Disk temperature'**
  String get diskTemperature;

  /// No description provided for @networkReceive.
  ///
  /// In en, this message translates to:
  /// **'Network receive rate'**
  String get networkReceive;

  /// No description provided for @networkSend.
  ///
  /// In en, this message translates to:
  /// **'Network send rate'**
  String get networkSend;

  /// No description provided for @errors.
  ///
  /// In en, this message translates to:
  /// **'Errors'**
  String get errors;

  /// No description provided for @warnings.
  ///
  /// In en, this message translates to:
  /// **'Warnings'**
  String get warnings;

  /// No description provided for @checkOk.
  ///
  /// In en, this message translates to:
  /// **'OK'**
  String get checkOk;

  /// No description provided for @checkWarning.
  ///
  /// In en, this message translates to:
  /// **'Warning'**
  String get checkWarning;

  /// No description provided for @checkError.
  ///
  /// In en, this message translates to:
  /// **'Error'**
  String get checkError;

  /// No description provided for @checkInfo.
  ///
  /// In en, this message translates to:
  /// **'Information'**
  String get checkInfo;

  /// No description provided for @mode.
  ///
  /// In en, this message translates to:
  /// **'Mode'**
  String get mode;

  /// No description provided for @preview.
  ///
  /// In en, this message translates to:
  /// **'Preview'**
  String get preview;

  /// No description provided for @executed.
  ///
  /// In en, this message translates to:
  /// **'Executed'**
  String get executed;

  /// No description provided for @duration.
  ///
  /// In en, this message translates to:
  /// **'Duration'**
  String get duration;

  /// No description provided for @freedBytes.
  ///
  /// In en, this message translates to:
  /// **'Reclaimed space'**
  String get freedBytes;

  /// No description provided for @deleted.
  ///
  /// In en, this message translates to:
  /// **'Deleted entries'**
  String get deleted;

  /// No description provided for @skipped.
  ///
  /// In en, this message translates to:
  /// **'Skipped entries'**
  String get skipped;

  /// No description provided for @failed.
  ///
  /// In en, this message translates to:
  /// **'Failed entries'**
  String get failed;

  /// No description provided for @channel.
  ///
  /// In en, this message translates to:
  /// **'Channel'**
  String get channel;

  /// No description provided for @result.
  ///
  /// In en, this message translates to:
  /// **'Result'**
  String get result;

  /// No description provided for @exitClass.
  ///
  /// In en, this message translates to:
  /// **'Exit classification'**
  String get exitClass;

  /// No description provided for @branch.
  ///
  /// In en, this message translates to:
  /// **'Branch'**
  String get branch;

  /// No description provided for @upstream.
  ///
  /// In en, this message translates to:
  /// **'Upstream'**
  String get upstream;

  /// No description provided for @aheadBehind.
  ///
  /// In en, this message translates to:
  /// **'Ahead / behind'**
  String get aheadBehind;

  /// No description provided for @githubUnavailable.
  ///
  /// In en, this message translates to:
  /// **'Not available'**
  String get githubUnavailable;

  /// No description provided for @githubComparisonNotApplicable.
  ///
  /// In en, this message translates to:
  /// **'Not applicable'**
  String get githubComparisonNotApplicable;

  /// No description provided for @staged.
  ///
  /// In en, this message translates to:
  /// **'Staged changes'**
  String get staged;

  /// No description provided for @unstaged.
  ///
  /// In en, this message translates to:
  /// **'Unstaged changes'**
  String get unstaged;

  /// No description provided for @untracked.
  ///
  /// In en, this message translates to:
  /// **'Untracked files'**
  String get untracked;

  /// No description provided for @conflicts.
  ///
  /// In en, this message translates to:
  /// **'Conflicts'**
  String get conflicts;

  /// No description provided for @remoteFreshness.
  ///
  /// In en, this message translates to:
  /// **'Remote data freshness'**
  String get remoteFreshness;

  /// No description provided for @repositoryPath.
  ///
  /// In en, this message translates to:
  /// **'Absolute repository path'**
  String get repositoryPath;

  /// No description provided for @lastRun.
  ///
  /// In en, this message translates to:
  /// **'Last run'**
  String get lastRun;

  /// No description provided for @updatePlan.
  ///
  /// In en, this message translates to:
  /// **'Update plan'**
  String get updatePlan;

  /// No description provided for @moduleRefresh.
  ///
  /// In en, this message translates to:
  /// **'Refresh'**
  String get moduleRefresh;

  /// No description provided for @moduleCancel.
  ///
  /// In en, this message translates to:
  /// **'Cancel request'**
  String get moduleCancel;

  /// No description provided for @moduleDiagnostics.
  ///
  /// In en, this message translates to:
  /// **'Diagnostics'**
  String get moduleDiagnostics;

  /// No description provided for @moduleInputError.
  ///
  /// In en, this message translates to:
  /// **'Check the repository path or selected entry.'**
  String get moduleInputError;

  /// No description provided for @moduleNetworkError.
  ///
  /// In en, this message translates to:
  /// **'The local tool did not respond in time.'**
  String get moduleNetworkError;

  /// No description provided for @moduleResponseError.
  ///
  /// In en, this message translates to:
  /// **'The local tool returned an unsupported response.'**
  String get moduleResponseError;

  /// No description provided for @moduleCancelled.
  ///
  /// In en, this message translates to:
  /// **'Request cancelled.'**
  String get moduleCancelled;

  /// No description provided for @moduleShutdownUnconfirmed.
  ///
  /// In en, this message translates to:
  /// **'Shutdown could not be confirmed. Access remains closed.'**
  String get moduleShutdownUnconfirmed;

  /// No description provided for @missingReading.
  ///
  /// In en, this message translates to:
  /// **'No reading'**
  String get missingReading;

  /// No description provided for @textPreviewLimit.
  ///
  /// In en, this message translates to:
  /// **'Text preview, up to 16 KiB.'**
  String get textPreviewLimit;

  /// No description provided for @gdsInstall.
  ///
  /// In en, this message translates to:
  /// **'GDS installation'**
  String get gdsInstall;

  /// No description provided for @verifiedApplications.
  ///
  /// In en, this message translates to:
  /// **'Verified applications'**
  String get verifiedApplications;

  /// No description provided for @aptObserve.
  ///
  /// In en, this message translates to:
  /// **'APT package observation'**
  String get aptObserve;

  /// No description provided for @toolchainObserve.
  ///
  /// In en, this message translates to:
  /// **'Toolchain observation'**
  String get toolchainObserve;

  /// No description provided for @connected.
  ///
  /// In en, this message translates to:
  /// **'Connected'**
  String get connected;

  /// No description provided for @connecting.
  ///
  /// In en, this message translates to:
  /// **'Connecting'**
  String get connecting;

  /// No description provided for @diagnosticsUnavailable.
  ///
  /// In en, this message translates to:
  /// **'Local diagnostics are unavailable. Sign-in can continue.'**
  String get diagnosticsUnavailable;

  /// No description provided for @localDiagnosticsOnly.
  ///
  /// In en, this message translates to:
  /// **'Local diagnostics are initialized. Server delivery status is shown under Clipboard and alerts.'**
  String get localDiagnosticsOnly;

  /// No description provided for @diagnosticsDegraded.
  ///
  /// In en, this message translates to:
  /// **'Diagnostics: rejected {rejected}, dropped {dropped}, write failures {failures}, pending {pending}.'**
  String diagnosticsDegraded(
    String rejected,
    String dropped,
    String failures,
    String pending,
  );

  /// No description provided for @startupFailed.
  ///
  /// In en, this message translates to:
  /// **'NDS could not start. Restart the application. If this continues, check the installation and local diagnostics.'**
  String get startupFailed;

  /// No description provided for @diagnosticsHttp.
  ///
  /// In en, this message translates to:
  /// **'Network diagnostics'**
  String get diagnosticsHttp;

  /// No description provided for @diagnosticsIo.
  ///
  /// In en, this message translates to:
  /// **'Local operations'**
  String get diagnosticsIo;

  /// No description provided for @devicesTitle.
  ///
  /// In en, this message translates to:
  /// **'Your devices'**
  String get devicesTitle;

  /// No description provided for @devicesHelp.
  ///
  /// In en, this message translates to:
  /// **'Each installation has its own signing key. Active enrolled devices receive access to the vault.'**
  String get devicesHelp;

  /// No description provided for @devicesUnverified.
  ///
  /// In en, this message translates to:
  /// **'Device inventory has not been freshly verified.'**
  String get devicesUnverified;

  /// No description provided for @deviceName.
  ///
  /// In en, this message translates to:
  /// **'Device name'**
  String get deviceName;

  /// No description provided for @enrollDevice.
  ///
  /// In en, this message translates to:
  /// **'Register this device'**
  String get enrollDevice;

  /// No description provided for @refreshDevices.
  ///
  /// In en, this message translates to:
  /// **'Refresh devices'**
  String get refreshDevices;

  /// No description provided for @thisDevice.
  ///
  /// In en, this message translates to:
  /// **'This installation'**
  String get thisDevice;

  /// No description provided for @deviceActive.
  ///
  /// In en, this message translates to:
  /// **'Active'**
  String get deviceActive;

  /// No description provided for @deviceRevoked.
  ///
  /// In en, this message translates to:
  /// **'Revoked'**
  String get deviceRevoked;

  /// No description provided for @deviceCreated.
  ///
  /// In en, this message translates to:
  /// **'Registered'**
  String get deviceCreated;

  /// No description provided for @deviceRevokedHelp.
  ///
  /// In en, this message translates to:
  /// **'This device key has been revoked. It is retained locally and cannot be reused for enrollment.'**
  String get deviceRevokedHelp;

  /// No description provided for @revokeDevice.
  ///
  /// In en, this message translates to:
  /// **'Revoke device'**
  String get revokeDevice;

  /// No description provided for @revokeDeviceHelp.
  ///
  /// In en, this message translates to:
  /// **'Revoke access for {name}? This identity cannot be enrolled again with the same key.'**
  String revokeDeviceHelp(String name);

  /// No description provided for @keepDevice.
  ///
  /// In en, this message translates to:
  /// **'Keep device'**
  String get keepDevice;

  /// No description provided for @vaultTitle.
  ///
  /// In en, this message translates to:
  /// **'Vault and sync'**
  String get vaultTitle;

  /// No description provided for @vaultHelp.
  ///
  /// In en, this message translates to:
  /// **'Your vault opens automatically after sign-in. Changes synchronize through the server; secret values stay in native storage and encrypted records.'**
  String get vaultHelp;

  /// No description provided for @vaultLocked.
  ///
  /// In en, this message translates to:
  /// **'Vault access is unavailable. Verify the session, device and server connection.'**
  String get vaultLocked;

  /// No description provided for @vaultUnlocked.
  ///
  /// In en, this message translates to:
  /// **'Protected vault key is available.'**
  String get vaultUnlocked;

  /// No description provided for @vaultCounts.
  ///
  /// In en, this message translates to:
  /// **'Pending: {pending} · Conflicts: {conflicts}'**
  String vaultCounts(int pending, int conflicts);

  /// No description provided for @vaultKeyIdentity.
  ///
  /// In en, this message translates to:
  /// **'Key identity'**
  String get vaultKeyIdentity;

  /// No description provided for @vaultRecordTitle.
  ///
  /// In en, this message translates to:
  /// **'Record title'**
  String get vaultRecordTitle;

  /// No description provided for @vaultUsername.
  ///
  /// In en, this message translates to:
  /// **'Username or account label'**
  String get vaultUsername;

  /// No description provided for @vaultSecret.
  ///
  /// In en, this message translates to:
  /// **'Secret value'**
  String get vaultSecret;

  /// No description provided for @vaultSave.
  ///
  /// In en, this message translates to:
  /// **'Encrypt and queue for sync'**
  String get vaultSave;

  /// No description provided for @vaultRevision.
  ///
  /// In en, this message translates to:
  /// **'Revision {revision}'**
  String vaultRevision(String revision);

  /// No description provided for @vaultConflict.
  ///
  /// In en, this message translates to:
  /// **'Unresolved conflict. Both versions are retained.'**
  String get vaultConflict;

  /// No description provided for @vaultSync.
  ///
  /// In en, this message translates to:
  /// **'Synchronize now'**
  String get vaultSync;

  /// No description provided for @vaultInputError.
  ///
  /// In en, this message translates to:
  /// **'Check the record fields.'**
  String get vaultInputError;

  /// No description provided for @vaultProofError.
  ///
  /// In en, this message translates to:
  /// **'The vault key or record could not be verified.'**
  String get vaultProofError;

  /// No description provided for @syncedState.
  ///
  /// In en, this message translates to:
  /// **'Synced device and module state'**
  String get syncedState;

  /// No description provided for @publishDeviceState.
  ///
  /// In en, this message translates to:
  /// **'Publish device, session and module state'**
  String get publishDeviceState;

  /// No description provided for @stateObserved.
  ///
  /// In en, this message translates to:
  /// **'Observed at'**
  String get stateObserved;

  /// No description provided for @stateChecked.
  ///
  /// In en, this message translates to:
  /// **'Last checked'**
  String get stateChecked;

  /// No description provided for @publishModuleState.
  ///
  /// In en, this message translates to:
  /// **'Publish observed module state'**
  String get publishModuleState;

  /// No description provided for @stateHelp.
  ///
  /// In en, this message translates to:
  /// **'These are encrypted observations from signed devices. Their timestamps describe when they were observed, not current reachability. Sync does not execute tools on another device.'**
  String get stateHelp;

  /// No description provided for @statePublishError.
  ///
  /// In en, this message translates to:
  /// **'State could not be synchronized. Check the session, device and vault connection.'**
  String get statePublishError;

  /// No description provided for @stateDevice.
  ///
  /// In en, this message translates to:
  /// **'Device'**
  String get stateDevice;

  /// No description provided for @stateDevices.
  ///
  /// In en, this message translates to:
  /// **'Devices'**
  String get stateDevices;

  /// No description provided for @vaultRegistrationRequired.
  ///
  /// In en, this message translates to:
  /// **'An existing vault key must first be registered by an updated installation that already has it.'**
  String get vaultRegistrationRequired;

  /// No description provided for @syncCatchingUp.
  ///
  /// In en, this message translates to:
  /// **'Catching up with the server…'**
  String get syncCatchingUp;

  /// No description provided for @syncNotObserved.
  ///
  /// In en, this message translates to:
  /// **'Synchronization has not been observed yet.'**
  String get syncNotObserved;

  /// No description provided for @syncCaughtUp.
  ///
  /// In en, this message translates to:
  /// **'Caught up at the last successful check.'**
  String get syncCaughtUp;

  /// No description provided for @passkey.
  ///
  /// In en, this message translates to:
  /// **'Sign in with passkey'**
  String get passkey;

  /// No description provided for @registerPasskey.
  ///
  /// In en, this message translates to:
  /// **'Create passkey'**
  String get registerPasskey;

  /// No description provided for @openPasskey.
  ///
  /// In en, this message translates to:
  /// **'Open passkey verification'**
  String get openPasskey;

  /// No description provided for @passkeyHelp.
  ///
  /// In en, this message translates to:
  /// **'Compare this code in the browser, then verify your passkey. Creating a passkey requires a recent sign-in.'**
  String get passkeyHelp;

  /// No description provided for @utilitiesTitle.
  ///
  /// In en, this message translates to:
  /// **'Clipboard and alerts'**
  String get utilitiesTitle;

  /// No description provided for @sharedClipboard.
  ///
  /// In en, this message translates to:
  /// **'Shared clipboard'**
  String get sharedClipboard;

  /// No description provided for @alertsTitle.
  ///
  /// In en, this message translates to:
  /// **'Alerts'**
  String get alertsTitle;

  /// No description provided for @clipboardHelp.
  ///
  /// In en, this message translates to:
  /// **'Text is encrypted and synchronizes automatically with the server. Share only the text you intend to send; sensitive clipboard items and URI data are excluded.'**
  String get clipboardHelp;

  /// No description provided for @sharedText.
  ///
  /// In en, this message translates to:
  /// **'Text to share'**
  String get sharedText;

  /// No description provided for @shareText.
  ///
  /// In en, this message translates to:
  /// **'Share text'**
  String get shareText;

  /// No description provided for @shareNativeClipboard.
  ///
  /// In en, this message translates to:
  /// **'Share device clipboard'**
  String get shareNativeClipboard;

  /// No description provided for @clipboardArchive.
  ///
  /// In en, this message translates to:
  /// **'Choose from clipboard archive'**
  String get clipboardArchive;

  /// No description provided for @clipboardReady.
  ///
  /// In en, this message translates to:
  /// **'Server revision {sequence} · {bytes} bytes'**
  String clipboardReady(int sequence, int bytes);

  /// No description provided for @clipboardEmpty.
  ///
  /// In en, this message translates to:
  /// **'No committed shared text yet'**
  String get clipboardEmpty;

  /// No description provided for @copySharedClipboard.
  ///
  /// In en, this message translates to:
  /// **'Copy shared text'**
  String get copySharedClipboard;

  /// No description provided for @utilityError.
  ///
  /// In en, this message translates to:
  /// **'Operation unavailable: {code}'**
  String utilityError(String code);

  /// No description provided for @telemetryDelivery.
  ///
  /// In en, this message translates to:
  /// **'Queued {pending} · Received {delivered} · Lost {dropped} · Attempts failed {failures}'**
  String telemetryDelivery(
    int pending,
    int delivered,
    int dropped,
    int failures,
  );

  /// No description provided for @telemetryInitialized.
  ///
  /// In en, this message translates to:
  /// **'Client delivery is active. Received counts confirm server acceptance.'**
  String get telemetryInitialized;

  /// No description provided for @telemetryUnavailable.
  ///
  /// In en, this message translates to:
  /// **'Client delivery is unavailable; local diagnostics remain active.'**
  String get telemetryUnavailable;

  /// No description provided for @noIncidents.
  ///
  /// In en, this message translates to:
  /// **'No incidents'**
  String get noIncidents;

  /// No description provided for @incidentState.
  ///
  /// In en, this message translates to:
  /// **'{kind} · {state} · {occurrences} occurrences'**
  String incidentState(String kind, String state, int occurrences);

  /// No description provided for @acknowledgeIncident.
  ///
  /// In en, this message translates to:
  /// **'Acknowledge'**
  String get acknowledgeIncident;

  /// No description provided for @silenceIncident.
  ///
  /// In en, this message translates to:
  /// **'Silence for one hour'**
  String get silenceIncident;

  /// No description provided for @moreIncidents.
  ///
  /// In en, this message translates to:
  /// **'Next incidents'**
  String get moreIncidents;

  /// No description provided for @syncLastSeen.
  ///
  /// In en, this message translates to:
  /// **'Last complete sync: {time}'**
  String syncLastSeen(String time);

  /// No description provided for @vaultDelete.
  ///
  /// In en, this message translates to:
  /// **'Delete record'**
  String get vaultDelete;

  /// No description provided for @vaultDeleteHelp.
  ///
  /// In en, this message translates to:
  /// **'Delete “{title}” on all synchronized devices?'**
  String vaultDeleteHelp(String title);

  /// No description provided for @vaultResolve.
  ///
  /// In en, this message translates to:
  /// **'Resolve conflict'**
  String get vaultResolve;

  /// No description provided for @vaultResolveHelp.
  ///
  /// In en, this message translates to:
  /// **'Apply “{choice}”? Other devices will receive this decision after synchronization.'**
  String vaultResolveHelp(String choice);

  /// No description provided for @operationBusy.
  ///
  /// In en, this message translates to:
  /// **'Another operation is in progress or this view changed. Refresh and try again.'**
  String get operationBusy;

  /// No description provided for @vaultEditing.
  ///
  /// In en, this message translates to:
  /// **'Editing: {title}. Enter the secret value to save.'**
  String vaultEditing(String title);

  /// No description provided for @vaultDeleted.
  ///
  /// In en, this message translates to:
  /// **'Deleted record'**
  String get vaultDeleted;

  /// No description provided for @vaultConflictingTitle.
  ///
  /// In en, this message translates to:
  /// **'Competing version: {title}'**
  String vaultConflictingTitle(String title);

  /// No description provided for @vaultConflictingDelete.
  ///
  /// In en, this message translates to:
  /// **'The competing version deletes this record.'**
  String get vaultConflictingDelete;

  /// No description provided for @vaultKeepServer.
  ///
  /// In en, this message translates to:
  /// **'Keep server version'**
  String get vaultKeepServer;

  /// No description provided for @vaultUseConflict.
  ///
  /// In en, this message translates to:
  /// **'Use competing version'**
  String get vaultUseConflict;

  /// No description provided for @vaultCopySecret.
  ///
  /// In en, this message translates to:
  /// **'Copy secret'**
  String get vaultCopySecret;

  /// No description provided for @vaultEdit.
  ///
  /// In en, this message translates to:
  /// **'Edit'**
  String get vaultEdit;

  /// No description provided for @vaultNextPage.
  ///
  /// In en, this message translates to:
  /// **'Next records'**
  String get vaultNextPage;

  /// No description provided for @vaultFirstPage.
  ///
  /// In en, this message translates to:
  /// **'First records'**
  String get vaultFirstPage;

  /// No description provided for @passkeysTitle.
  ///
  /// In en, this message translates to:
  /// **'Your passkeys'**
  String get passkeysTitle;

  /// No description provided for @passkeysHelp.
  ///
  /// In en, this message translates to:
  /// **'Removing a passkey ends sessions created with it. Removal requires signing in within the last five minutes.'**
  String get passkeysHelp;

  /// No description provided for @passkeysEmpty.
  ///
  /// In en, this message translates to:
  /// **'No passkeys registered.'**
  String get passkeysEmpty;

  /// No description provided for @passkeyCreated.
  ///
  /// In en, this message translates to:
  /// **'Created: {time}'**
  String passkeyCreated(String time);

  /// No description provided for @passkeyRemove.
  ///
  /// In en, this message translates to:
  /// **'Remove passkey'**
  String get passkeyRemove;

  /// No description provided for @passkeyRemoveHelp.
  ///
  /// In en, this message translates to:
  /// **'Remove passkey {id}? Sessions created with this key will be revoked, possibly including this session.'**
  String passkeyRemoveHelp(String id);

  /// No description provided for @passkeyFreshLogin.
  ///
  /// In en, this message translates to:
  /// **'Sign in again before changing passkeys.'**
  String get passkeyFreshLogin;

  /// No description provided for @keepPasskey.
  ///
  /// In en, this message translates to:
  /// **'Keep passkey'**
  String get keepPasskey;

  /// No description provided for @passkeyMethod.
  ///
  /// In en, this message translates to:
  /// **'Passkey'**
  String get passkeyMethod;

  /// No description provided for @actionCancel.
  ///
  /// In en, this message translates to:
  /// **'Cancel'**
  String get actionCancel;

  /// No description provided for @exportUnknown.
  ///
  /// In en, this message translates to:
  /// **'Server export policy has not been confirmed.'**
  String get exportUnknown;

  /// No description provided for @exportEnabled.
  ///
  /// In en, this message translates to:
  /// **'Server export is enabled.'**
  String get exportEnabled;

  /// No description provided for @exportDisabled.
  ///
  /// In en, this message translates to:
  /// **'The operator disabled server export.'**
  String get exportDisabled;

  /// No description provided for @exportPaused.
  ///
  /// In en, this message translates to:
  /// **'Delivery is paused. Return to the app to resume.'**
  String get exportPaused;

  /// No description provided for @eventsLabel.
  ///
  /// In en, this message translates to:
  /// **'Events'**
  String get eventsLabel;

  /// No description provided for @spansLabel.
  ///
  /// In en, this message translates to:
  /// **'Operation traces'**
  String get spansLabel;

  /// No description provided for @exportLastSeen.
  ///
  /// In en, this message translates to:
  /// **'Last server acceptance: {time}'**
  String exportLastSeen(String time);

  /// No description provided for @incidentsObserved.
  ///
  /// In en, this message translates to:
  /// **'Alert state observed: {time}'**
  String incidentsObserved(String time);

  /// No description provided for @incidentsUnavailable.
  ///
  /// In en, this message translates to:
  /// **'Alert state could not be refreshed. Displayed entries may be outdated.'**
  String get incidentsUnavailable;

  /// No description provided for @clipboardUnavailable.
  ///
  /// In en, this message translates to:
  /// **'Shared clipboard state could not be refreshed.'**
  String get clipboardUnavailable;

  /// No description provided for @clipboardRejected.
  ///
  /// In en, this message translates to:
  /// **'This clipboard item is private, unsupported or too large to share. Choose plain text up to 8 KiB.'**
  String get clipboardRejected;

  /// No description provided for @clipboardCopied.
  ///
  /// In en, this message translates to:
  /// **'Copied to device clipboard.'**
  String get clipboardCopied;

  /// No description provided for @offlineAuthorized.
  ///
  /// In en, this message translates to:
  /// **'Offline access uses the last verified session and device. Changes remain on this device until synchronization.'**
  String get offlineAuthorized;

  /// No description provided for @storageFull.
  ///
  /// In en, this message translates to:
  /// **'Local storage is full. The change was not saved.'**
  String get storageFull;

  /// No description provided for @logoutPending.
  ///
  /// In en, this message translates to:
  /// **'Signed out locally. Server sign-out will complete after reconnecting.'**
  String get logoutPending;

  /// No description provided for @recentAuthenticationRequired.
  ///
  /// In en, this message translates to:
  /// **'Sign in again to manage passkeys. Your current session remains valid.'**
  String get recentAuthenticationRequired;

  /// No description provided for @browserPrivacy.
  ///
  /// In en, this message translates to:
  /// **'The session stays in a protected browser cookie. Vault keys are encrypted in this browser profile; the server delivers them after verified sign-in and device enrollment.'**
  String get browserPrivacy;

  /// No description provided for @browserVaultHelp.
  ///
  /// In en, this message translates to:
  /// **'Your vault opens automatically after sign-in. Changes are stored encrypted in this browser and synchronize through the server. Keep this browser’s site data until all changes are synchronized.'**
  String get browserVaultHelp;

  /// No description provided for @browserStoragePersistent.
  ///
  /// In en, this message translates to:
  /// **'The browser granted persistent storage. Clearing site data still removes local changes.'**
  String get browserStoragePersistent;

  /// No description provided for @browserStorageEvictable.
  ///
  /// In en, this message translates to:
  /// **'The browser may remove site data under storage pressure. Unsynchronized changes would be lost.'**
  String get browserStorageEvictable;

  /// No description provided for @vaultRecordPending.
  ///
  /// In en, this message translates to:
  /// **'Local change awaiting synchronization'**
  String get vaultRecordPending;

  /// No description provided for @telemetryPersistenceReady.
  ///
  /// In en, this message translates to:
  /// **'Browser log storage has confirmed writes.'**
  String get telemetryPersistenceReady;

  /// No description provided for @telemetryPersistenceUnavailable.
  ///
  /// In en, this message translates to:
  /// **'Durable browser log storage has not confirmed a successful write. Unstored events can be lost when this page closes.'**
  String get telemetryPersistenceUnavailable;

  /// No description provided for @vaultStorageUnavailable.
  ///
  /// In en, this message translates to:
  /// **'Local encrypted storage is unavailable. Check this browser or device’s storage permissions and support.'**
  String get vaultStorageUnavailable;

  /// No description provided for @vaultDependencyUnavailable.
  ///
  /// In en, this message translates to:
  /// **'The vault could not access required local storage or server services.'**
  String get vaultDependencyUnavailable;

  /// No description provided for @serverCapacityExhausted.
  ///
  /// In en, this message translates to:
  /// **'Server storage capacity is exhausted. Your encrypted changes remain on this device. Automatic sending is paused while NDS checks for a safe retry; you can also retry manually.'**
  String get serverCapacityExhausted;

  /// No description provided for @catalogTitle.
  ///
  /// In en, this message translates to:
  /// **'GitHub repositories'**
  String get catalogTitle;

  /// No description provided for @catalogHelp.
  ///
  /// In en, this message translates to:
  /// **'Choose a vault record containing a GitHub access token to update the saved catalog. Each request reads one page available to that credential.'**
  String get catalogHelp;

  /// No description provided for @catalogCredential.
  ///
  /// In en, this message translates to:
  /// **'Vault credential'**
  String get catalogCredential;

  /// No description provided for @catalogRefreshCredentials.
  ///
  /// In en, this message translates to:
  /// **'Refresh vault records'**
  String get catalogRefreshCredentials;

  /// No description provided for @catalogMoreCredentials.
  ///
  /// In en, this message translates to:
  /// **'Next vault records'**
  String get catalogMoreCredentials;

  /// No description provided for @catalogNoCredentials.
  ///
  /// In en, this message translates to:
  /// **'No selectable records on this vault page. Open the vault to add or resolve a credential record.'**
  String get catalogNoCredentials;

  /// No description provided for @catalogRead.
  ///
  /// In en, this message translates to:
  /// **'Read first page'**
  String get catalogRead;

  /// No description provided for @catalogNextPage.
  ///
  /// In en, this message translates to:
  /// **'Next repository page'**
  String get catalogNextPage;

  /// No description provided for @catalogChooseCredential.
  ///
  /// In en, this message translates to:
  /// **'Choose a current vault record.'**
  String get catalogChooseCredential;

  /// No description provided for @catalogSignIn.
  ///
  /// In en, this message translates to:
  /// **'Sign in and verify this device to read repositories.'**
  String get catalogSignIn;

  /// No description provided for @catalogCredentialChanged.
  ///
  /// In en, this message translates to:
  /// **'This vault record changed. Refresh records and select it again.'**
  String get catalogCredentialChanged;

  /// No description provided for @catalogInvalidResponse.
  ///
  /// In en, this message translates to:
  /// **'Repository data could not be verified.'**
  String get catalogInvalidResponse;

  /// No description provided for @catalogProviderFailed.
  ///
  /// In en, this message translates to:
  /// **'GitHub could not return this page. Check the credential’s access and try again.'**
  String get catalogProviderFailed;

  /// No description provided for @catalogUnavailable.
  ///
  /// In en, this message translates to:
  /// **'Repository catalog is unavailable.'**
  String get catalogUnavailable;

  /// No description provided for @catalogObservationUnknown.
  ///
  /// In en, this message translates to:
  /// **'Observation time is unavailable.'**
  String get catalogObservationUnknown;

  /// No description provided for @catalogObserved.
  ///
  /// In en, this message translates to:
  /// **'Observed: {time}'**
  String catalogObserved(String time);

  /// No description provided for @catalogPrincipal.
  ///
  /// In en, this message translates to:
  /// **'GitHub account: {login}'**
  String catalogPrincipal(String login);

  /// No description provided for @catalogPageScope.
  ///
  /// In en, this message translates to:
  /// **'Only repositories available to the selected credential are shown. Each request replaces this page.'**
  String get catalogPageScope;

  /// No description provided for @catalogEmptyPage.
  ///
  /// In en, this message translates to:
  /// **'No repositories on this page.'**
  String get catalogEmptyPage;

  /// No description provided for @catalogOwnerIgnored.
  ///
  /// In en, this message translates to:
  /// **'Ignored on this device by the owner\'s list. Saved data is kept; it is not discovered, connected or synchronized.'**
  String get catalogOwnerIgnored;

  /// No description provided for @catalogAccountIgnored.
  ///
  /// In en, this message translates to:
  /// **'This GitHub account is on this device\'s ignored-account list.'**
  String get catalogAccountIgnored;

  /// No description provided for @catalogIgnoreListInvalid.
  ///
  /// In en, this message translates to:
  /// **'The ignored-account list on this device cannot be read. Nothing is included until it is fixed.'**
  String get catalogIgnoreListInvalid;

  /// No description provided for @catalogRepositoryIgnored.
  ///
  /// In en, this message translates to:
  /// **'This repository is on this device\'s ignored-repository list.'**
  String get catalogRepositoryIgnored;

  /// No description provided for @catalogRepositoryIgnoredLabel.
  ///
  /// In en, this message translates to:
  /// **'Ignored on this device'**
  String get catalogRepositoryIgnoredLabel;

  /// No description provided for @catalogOrganization.
  ///
  /// In en, this message translates to:
  /// **'Organization'**
  String get catalogOrganization;

  /// No description provided for @catalogPersonal.
  ///
  /// In en, this message translates to:
  /// **'Personal account'**
  String get catalogPersonal;

  /// No description provided for @catalogPublic.
  ///
  /// In en, this message translates to:
  /// **'Public'**
  String get catalogPublic;

  /// No description provided for @catalogPrivate.
  ///
  /// In en, this message translates to:
  /// **'Private'**
  String get catalogPrivate;

  /// No description provided for @catalogInternal.
  ///
  /// In en, this message translates to:
  /// **'Internal'**
  String get catalogInternal;

  /// No description provided for @catalogArchived.
  ///
  /// In en, this message translates to:
  /// **'Archived'**
  String get catalogArchived;

  /// No description provided for @catalogHasMore.
  ///
  /// In en, this message translates to:
  /// **'More repositories are available in the next page.'**
  String get catalogHasMore;

  /// No description provided for @catalogAccessEnd.
  ///
  /// In en, this message translates to:
  /// **'End of the list available to this credential. Other access scopes may include more repositories.'**
  String get catalogAccessEnd;

  /// No description provided for @catalogLimitReached.
  ///
  /// In en, this message translates to:
  /// **'Local page limit reached. GitHub still reports more repositories; this listing is incomplete.'**
  String get catalogLimitReached;

  /// No description provided for @catalogCancelled.
  ///
  /// In en, this message translates to:
  /// **'Repository request cancelled.'**
  String get catalogCancelled;

  /// No description provided for @catalogTimeout.
  ///
  /// In en, this message translates to:
  /// **'Repository request timed out. Try again.'**
  String get catalogTimeout;

  /// No description provided for @catalogPaused.
  ///
  /// In en, this message translates to:
  /// **'Repository requests are paused while this app is in the background.'**
  String get catalogPaused;

  /// No description provided for @screenView.
  ///
  /// In en, this message translates to:
  /// **'View screen'**
  String get screenView;

  /// No description provided for @screenEnded.
  ///
  /// In en, this message translates to:
  /// **'Screen viewing ended.'**
  String get screenEnded;

  /// No description provided for @screenOpening.
  ///
  /// In en, this message translates to:
  /// **'Waiting for screen access…'**
  String get screenOpening;

  /// No description provided for @screenUnavailable.
  ///
  /// In en, this message translates to:
  /// **'The screen is unavailable.'**
  String get screenUnavailable;

  /// No description provided for @screenStop.
  ///
  /// In en, this message translates to:
  /// **'Stop viewing'**
  String get screenStop;

  /// No description provided for @screenRequests.
  ///
  /// In en, this message translates to:
  /// **'Screen requests'**
  String get screenRequests;

  /// No description provided for @screenRequestsHelp.
  ///
  /// In en, this message translates to:
  /// **'Refresh requests from devices allowed by your current company policy. Sharing starts only after you allow this request.'**
  String get screenRequestsHelp;

  /// No description provided for @screenConsent.
  ///
  /// In en, this message translates to:
  /// **'Allow this device to view your screen for the requested session?'**
  String get screenConsent;

  /// No description provided for @screenAllow.
  ///
  /// In en, this message translates to:
  /// **'Allow screen viewing'**
  String get screenAllow;

  /// No description provided for @screenAccessAllowed.
  ///
  /// In en, this message translates to:
  /// **'Screen access is allowed for this session.'**
  String get screenAccessAllowed;

  /// No description provided for @screenStopSharing.
  ///
  /// In en, this message translates to:
  /// **'Stop sharing'**
  String get screenStopSharing;

  /// No description provided for @catalogSaved.
  ///
  /// In en, this message translates to:
  /// **'Saved catalog'**
  String get catalogSaved;

  /// No description provided for @catalogProvider.
  ///
  /// In en, this message translates to:
  /// **'Update from GitHub'**
  String get catalogProvider;

  /// No description provided for @catalogSavedHelp.
  ///
  /// In en, this message translates to:
  /// **'Encrypted observations retained in NDS. Dates show when data was observed, not current GitHub access or freshness.'**
  String get catalogSavedHelp;

  /// No description provided for @catalogReadSaved.
  ///
  /// In en, this message translates to:
  /// **'Refresh saved page'**
  String get catalogReadSaved;

  /// No description provided for @catalogSavedNext.
  ///
  /// In en, this message translates to:
  /// **'Next saved page'**
  String get catalogSavedNext;

  /// No description provided for @catalogReadSavedAgain.
  ///
  /// In en, this message translates to:
  /// **'Refresh the saved catalog and try again.'**
  String get catalogReadSavedAgain;

  /// No description provided for @catalogMissingOwner.
  ///
  /// In en, this message translates to:
  /// **'Owner {id}: details have not synchronized yet.'**
  String catalogMissingOwner(String id);

  /// No description provided for @catalogCapacityReached.
  ///
  /// In en, this message translates to:
  /// **'The catalog could not finish updating because synchronization storage reached its limit. Saved observations remain available.'**
  String get catalogCapacityReached;

  /// No description provided for @companySpace.
  ///
  /// In en, this message translates to:
  /// **'Space'**
  String get companySpace;

  /// No description provided for @companyPersonal.
  ///
  /// In en, this message translates to:
  /// **'Personal space'**
  String get companyPersonal;

  /// No description provided for @companyPersonalHelp.
  ///
  /// In en, this message translates to:
  /// **'Organization membership does not change your personal storage owner. Select an organization to view its current membership.'**
  String get companyPersonalHelp;

  /// No description provided for @companyName.
  ///
  /// In en, this message translates to:
  /// **'Organization name'**
  String get companyName;

  /// No description provided for @companyCreate.
  ///
  /// In en, this message translates to:
  /// **'Create organization'**
  String get companyCreate;

  /// No description provided for @companyCreateHelp.
  ///
  /// In en, this message translates to:
  /// **'This action uses your personal identity and creates a separate organization with you as its first administrator.'**
  String get companyCreateHelp;

  /// No description provided for @companyReviewedUnknown.
  ///
  /// In en, this message translates to:
  /// **'Current authorized state was reviewed. Finishing the retained command does not undo any previously submitted change.'**
  String get companyReviewedUnknown;

  /// No description provided for @companyServerRequired.
  ///
  /// In en, this message translates to:
  /// **'This server does not support explicit organization contexts. No other company is selected automatically.'**
  String get companyServerRequired;

  /// No description provided for @companyContextClosed.
  ///
  /// In en, this message translates to:
  /// **'This context is closed. Refresh memberships or sign in again before another action.'**
  String get companyContextClosed;

  /// No description provided for @companyTitle.
  ///
  /// In en, this message translates to:
  /// **'Organizations'**
  String get companyTitle;

  /// No description provided for @companyPrivacy.
  ///
  /// In en, this message translates to:
  /// **'Organization administration is online. Your personal Vault, secrets and repositories keep their original owner.'**
  String get companyPrivacy;

  /// No description provided for @companyAdmin.
  ///
  /// In en, this message translates to:
  /// **'Administrator'**
  String get companyAdmin;

  /// No description provided for @companyUser.
  ///
  /// In en, this message translates to:
  /// **'User'**
  String get companyUser;

  /// No description provided for @companyCompleted.
  ///
  /// In en, this message translates to:
  /// **'The server confirmed the change. Refresh to see the current state.'**
  String get companyCompleted;

  /// No description provided for @companyLastAdmin.
  ///
  /// In en, this message translates to:
  /// **'The last active administrator cannot be removed or demoted.'**
  String get companyLastAdmin;

  /// No description provided for @companyConflict.
  ///
  /// In en, this message translates to:
  /// **'The member changed since this page was loaded. Review the current state.'**
  String get companyConflict;

  /// No description provided for @companyExpired.
  ///
  /// In en, this message translates to:
  /// **'The command has expired. Review the current state before a new action.'**
  String get companyExpired;

  /// No description provided for @companyInvitationUnavailable.
  ///
  /// In en, this message translates to:
  /// **'This invitation or recipient is no longer available for this action.'**
  String get companyInvitationUnavailable;

  /// No description provided for @companyDenied.
  ///
  /// In en, this message translates to:
  /// **'Your current membership does not allow this action.'**
  String get companyDenied;

  /// No description provided for @companyInvalid.
  ///
  /// In en, this message translates to:
  /// **'Check the organization name, email address and selected action.'**
  String get companyInvalid;

  /// No description provided for @companyPending.
  ///
  /// In en, this message translates to:
  /// **'The original command is retained for retry. If the connection failed, the server may have applied it. Review the current state before creating another command. Closing this screen does not resend it.'**
  String get companyPending;

  /// No description provided for @companyRetry.
  ///
  /// In en, this message translates to:
  /// **'Retry the same command'**
  String get companyRetry;

  /// No description provided for @companyReview.
  ///
  /// In en, this message translates to:
  /// **'Review state and finish this command'**
  String get companyReview;

  /// No description provided for @companyMembers.
  ///
  /// In en, this message translates to:
  /// **'Members'**
  String get companyMembers;

  /// No description provided for @companyActive.
  ///
  /// In en, this message translates to:
  /// **'Active'**
  String get companyActive;

  /// No description provided for @companyRemoved.
  ///
  /// In en, this message translates to:
  /// **'Removed'**
  String get companyRemoved;

  /// No description provided for @companyRemove.
  ///
  /// In en, this message translates to:
  /// **'Remove member'**
  String get companyRemove;

  /// No description provided for @companyChangeRole.
  ///
  /// In en, this message translates to:
  /// **'Change role'**
  String get companyChangeRole;

  /// No description provided for @companyChangeHelp.
  ///
  /// In en, this message translates to:
  /// **'Apply this membership change to {member}? Removing membership closes organization access; personal sessions and data remain separate.'**
  String companyChangeHelp(String member);

  /// No description provided for @companyConfirm.
  ///
  /// In en, this message translates to:
  /// **'Confirm'**
  String get companyConfirm;

  /// No description provided for @companyNextMembers.
  ///
  /// In en, this message translates to:
  /// **'Next members'**
  String get companyNextMembers;

  /// No description provided for @companyInvite.
  ///
  /// In en, this message translates to:
  /// **'Invite by email'**
  String get companyInvite;

  /// No description provided for @companyDeliveryHelp.
  ///
  /// In en, this message translates to:
  /// **'Sending an invitation, acceptance by the mail service and joining the company are separate steps. Links expire after seven days.'**
  String get companyDeliveryHelp;

  /// No description provided for @companyInvitePending.
  ///
  /// In en, this message translates to:
  /// **'Awaiting acceptance'**
  String get companyInvitePending;

  /// No description provided for @companyJoined.
  ///
  /// In en, this message translates to:
  /// **'Joined'**
  String get companyJoined;

  /// No description provided for @companyInviteExpired.
  ///
  /// In en, this message translates to:
  /// **'Expired'**
  String get companyInviteExpired;

  /// No description provided for @companyInviteRevoked.
  ///
  /// In en, this message translates to:
  /// **'Revoked'**
  String get companyInviteRevoked;

  /// No description provided for @companyDeliveryPending.
  ///
  /// In en, this message translates to:
  /// **'Mail: awaiting delivery result'**
  String get companyDeliveryPending;

  /// No description provided for @companyDeliveryAccepted.
  ///
  /// In en, this message translates to:
  /// **'Mail: accepted by the mail service'**
  String get companyDeliveryAccepted;

  /// No description provided for @companyDeliveryFailed.
  ///
  /// In en, this message translates to:
  /// **'Mail: not sent'**
  String get companyDeliveryFailed;

  /// No description provided for @companyDeliveryUnknown.
  ///
  /// In en, this message translates to:
  /// **'Mail: delivery result unknown'**
  String get companyDeliveryUnknown;

  /// No description provided for @companyRevoke.
  ///
  /// In en, this message translates to:
  /// **'Revoke invitation'**
  String get companyRevoke;

  /// No description provided for @companyNextInvitations.
  ///
  /// In en, this message translates to:
  /// **'Next invitations'**
  String get companyNextInvitations;

  /// No description provided for @invitationImport.
  ///
  /// In en, this message translates to:
  /// **'Open invitation'**
  String get invitationImport;

  /// No description provided for @invitationLink.
  ///
  /// In en, this message translates to:
  /// **'Invitation link'**
  String get invitationLink;

  /// No description provided for @invitationVerify.
  ///
  /// In en, this message translates to:
  /// **'Invitation loaded. Enter the invited email address and verify the code sent to it. The link alone does not sign you in.'**
  String get invitationVerify;

  /// No description provided for @invitationSignOut.
  ///
  /// In en, this message translates to:
  /// **'An invitation is waiting. Sign out first to verify the invited email address.'**
  String get invitationSignOut;

  /// No description provided for @passkeyAccountHint.
  ///
  /// In en, this message translates to:
  /// **'For passkey sign-in, enter the email address linked to your NDS account. With multiple members, an account must be selected.'**
  String get passkeyAccountHint;

  /// No description provided for @screenTargets.
  ///
  /// In en, this message translates to:
  /// **'Devices for screen viewing'**
  String get screenTargets;

  /// No description provided for @screenTargetsEmpty.
  ///
  /// In en, this message translates to:
  /// **'No active devices.'**
  String get screenTargetsEmpty;

  /// No description provided for @screenNotRegistered.
  ///
  /// In en, this message translates to:
  /// **'Open screen sharing on this device first.'**
  String get screenNotRegistered;

  /// No description provided for @screenTargetsFailed.
  ///
  /// In en, this message translates to:
  /// **'Could not load devices. Refresh to check access and connection.'**
  String get screenTargetsFailed;

  /// No description provided for @moduleManagementHelp.
  ///
  /// In en, this message translates to:
  /// **'Enable modules and choose their NDS permissions for this device. System or browser consent is still required when a feature uses it.'**
  String get moduleManagementHelp;

  /// No description provided for @moduleEnabled.
  ///
  /// In en, this message translates to:
  /// **'Module enabled'**
  String get moduleEnabled;

  /// No description provided for @modulePermissions.
  ///
  /// In en, this message translates to:
  /// **'NDS permissions'**
  String get modulePermissions;

  /// No description provided for @moduleSave.
  ///
  /// In en, this message translates to:
  /// **'Save module settings'**
  String get moduleSave;

  /// No description provided for @moduleSaved.
  ///
  /// In en, this message translates to:
  /// **'Settings saved on this device; synchronization continues separately.'**
  String get moduleSaved;

  /// No description provided for @moduleDisabled.
  ///
  /// In en, this message translates to:
  /// **'Enable this module to use its features.'**
  String get moduleDisabled;

  /// No description provided for @modulePermissionRequired.
  ///
  /// In en, this message translates to:
  /// **'Choose the required NDS permissions before using this feature.'**
  String get modulePermissionRequired;

  /// No description provided for @moduleConfigurationConflict.
  ///
  /// In en, this message translates to:
  /// **'Module settings conflict. No permission is granted until the conflict is resolved.'**
  String get moduleConfigurationConflict;

  /// No description provided for @moduleDeviceRequired.
  ///
  /// In en, this message translates to:
  /// **'Sign in and register this device to manage modules.'**
  String get moduleDeviceRequired;

  /// No description provided for @moduleNotInstalled.
  ///
  /// In en, this message translates to:
  /// **'The required local tool is not installed or has not been found.'**
  String get moduleNotInstalled;

  /// No description provided for @moduleInstalled.
  ///
  /// In en, this message translates to:
  /// **'Available on this device'**
  String get moduleInstalled;

  /// No description provided for @moduleInstallationUnknown.
  ///
  /// In en, this message translates to:
  /// **'Availability has not been checked'**
  String get moduleInstallationUnknown;

  /// No description provided for @moduleMetadataError.
  ///
  /// In en, this message translates to:
  /// **'Module settings could not be refreshed. The last operation result is unchanged.'**
  String get moduleMetadataError;

  /// No description provided for @permissionReadOnly.
  ///
  /// In en, this message translates to:
  /// **'Read local information'**
  String get permissionReadOnly;

  /// No description provided for @permissionCredentialStore.
  ///
  /// In en, this message translates to:
  /// **'Use protected credential references'**
  String get permissionCredentialStore;

  /// No description provided for @permissionProcessExecution.
  ///
  /// In en, this message translates to:
  /// **'Run the declared local tool'**
  String get permissionProcessExecution;

  /// No description provided for @permissionLocalNetwork.
  ///
  /// In en, this message translates to:
  /// **'Communicate with the declared service'**
  String get permissionLocalNetwork;

  /// No description provided for @permissionUserService.
  ///
  /// In en, this message translates to:
  /// **'Use the current user service'**
  String get permissionUserService;

  /// No description provided for @permissionClipboard.
  ///
  /// In en, this message translates to:
  /// **'Use clipboard features'**
  String get permissionClipboard;

  /// No description provided for @moduleSettingsChanged.
  ///
  /// In en, this message translates to:
  /// **'Module settings changed. Refresh the form before saving your choice.'**
  String get moduleSettingsChanged;

  /// No description provided for @moduleReloadDraft.
  ///
  /// In en, this message translates to:
  /// **'Reload settings'**
  String get moduleReloadDraft;

  /// No description provided for @catalogSourceHelp.
  ///
  /// In en, this message translates to:
  /// **'This choice is saved and synchronized across your account. Enable GitHub module permissions separately on each device.'**
  String get catalogSourceHelp;

  /// No description provided for @catalogClearSource.
  ///
  /// In en, this message translates to:
  /// **'Clear GitHub choice'**
  String get catalogClearSource;

  /// No description provided for @moduleStateQueued.
  ///
  /// In en, this message translates to:
  /// **'Local states queued for synchronization.'**
  String get moduleStateQueued;

  /// No description provided for @permissionGitWrite.
  ///
  /// In en, this message translates to:
  /// **'Create checkouts and synchronize existing Git commits'**
  String get permissionGitWrite;

  /// No description provided for @permissionBrowserControl.
  ///
  /// In en, this message translates to:
  /// **'Control agent browser tabs'**
  String get permissionBrowserControl;

  /// No description provided for @browserControlScope.
  ///
  /// In en, this message translates to:
  /// **'Allows agents to automate tabs in NDS groups. Other browser tabs remain outside this permission.'**
  String get browserControlScope;

  /// No description provided for @repositoryConnect.
  ///
  /// In en, this message translates to:
  /// **'Connect checkout'**
  String get repositoryConnect;

  /// No description provided for @repositoryConnected.
  ///
  /// In en, this message translates to:
  /// **'Checkout connected: {path}'**
  String repositoryConnected(String path);

  /// No description provided for @repositoryRegistrationPending.
  ///
  /// In en, this message translates to:
  /// **'Checkout exists at {path}. Retry connection to finish NDS registration.'**
  String repositoryRegistrationPending(String path);

  /// No description provided for @repositoryIntentQueued.
  ///
  /// In en, this message translates to:
  /// **'Connection intent was queued for NDS synchronization.'**
  String get repositoryIntentQueued;

  /// No description provided for @repositoryPathOccupied.
  ///
  /// In en, this message translates to:
  /// **'This location is already occupied. Existing files were preserved.'**
  String get repositoryPathOccupied;

  /// No description provided for @repositoryDifferentInstallation.
  ///
  /// In en, this message translates to:
  /// **'This checkout belongs to another NDS account or installation.'**
  String get repositoryDifferentInstallation;

  /// No description provided for @repositoryRelocationRequired.
  ///
  /// In en, this message translates to:
  /// **'The saved connection uses another location. Review its move before connecting here.'**
  String get repositoryRelocationRequired;

  /// No description provided for @repositoryWrongRemote.
  ///
  /// In en, this message translates to:
  /// **'The Git origin or stored repository identity does not match the selected repository.'**
  String get repositoryWrongRemote;

  /// No description provided for @repositoryUnsupported.
  ///
  /// In en, this message translates to:
  /// **'Local checkout connection is not supported on this device.'**
  String get repositoryUnsupported;

  /// No description provided for @repositorySyncRunning.
  ///
  /// In en, this message translates to:
  /// **'Synchronizing existing commits…'**
  String get repositorySyncRunning;

  /// No description provided for @repositorySyncCurrent.
  ///
  /// In en, this message translates to:
  /// **'Commits are up to date'**
  String get repositorySyncCurrent;

  /// No description provided for @repositorySyncPublished.
  ///
  /// In en, this message translates to:
  /// **'Existing commits published'**
  String get repositorySyncPublished;

  /// No description provided for @repositorySyncFastForwarded.
  ///
  /// In en, this message translates to:
  /// **'Checkout fast-forwarded'**
  String get repositorySyncFastForwarded;

  /// No description provided for @repositorySyncDirty.
  ///
  /// In en, this message translates to:
  /// **'Uncommitted changes preserved; synchronization paused for this checkout'**
  String get repositorySyncDirty;

  /// No description provided for @repositorySyncDiverged.
  ///
  /// In en, this message translates to:
  /// **'Histories diverged; changes preserved'**
  String get repositorySyncDiverged;

  /// No description provided for @repositorySyncRefused.
  ///
  /// In en, this message translates to:
  /// **'Synchronization refused; existing work preserved'**
  String get repositorySyncRefused;

  /// No description provided for @appUpdateTitle.
  ///
  /// In en, this message translates to:
  /// **'NDS application updates'**
  String get appUpdateTitle;

  /// No description provided for @appUpdateCheck.
  ///
  /// In en, this message translates to:
  /// **'Check for updates'**
  String get appUpdateCheck;

  /// No description provided for @appUpdateDownload.
  ///
  /// In en, this message translates to:
  /// **'Download update'**
  String get appUpdateDownload;

  /// No description provided for @appUpdateCancel.
  ///
  /// In en, this message translates to:
  /// **'Cancel download'**
  String get appUpdateCancel;

  /// No description provided for @appUpdateIdle.
  ///
  /// In en, this message translates to:
  /// **'Check for an application update.'**
  String get appUpdateIdle;

  /// No description provided for @appUpdateChecking.
  ///
  /// In en, this message translates to:
  /// **'Checking the update…'**
  String get appUpdateChecking;

  /// No description provided for @appUpdateAvailable.
  ///
  /// In en, this message translates to:
  /// **'A verified update is available to download.'**
  String get appUpdateAvailable;

  /// No description provided for @appUpdateDownloading.
  ///
  /// In en, this message translates to:
  /// **'Downloading the update…'**
  String get appUpdateDownloading;

  /// No description provided for @appUpdateStaged.
  ///
  /// In en, this message translates to:
  /// **'The download is verified and saved. Installation is not available in this build.'**
  String get appUpdateStaged;

  /// No description provided for @appUpdateUnsupported.
  ///
  /// In en, this message translates to:
  /// **'Application updates are not supported on this platform.'**
  String get appUpdateUnsupported;

  /// No description provided for @appUpdateNotConfigured.
  ///
  /// In en, this message translates to:
  /// **'Application updates are not configured for this build.'**
  String get appUpdateNotConfigured;

  /// No description provided for @appUpdateBusy.
  ///
  /// In en, this message translates to:
  /// **'Another update operation is running. Try again when it finishes.'**
  String get appUpdateBusy;

  /// No description provided for @appUpdateCancelled.
  ///
  /// In en, this message translates to:
  /// **'The download was cancelled.'**
  String get appUpdateCancelled;

  /// No description provided for @appUpdateTimeout.
  ///
  /// In en, this message translates to:
  /// **'The update operation timed out. Try again.'**
  String get appUpdateTimeout;

  /// No description provided for @appUpdateNetwork.
  ///
  /// In en, this message translates to:
  /// **'The update could not be retrieved from an approved source.'**
  String get appUpdateNetwork;

  /// No description provided for @appUpdateTooLarge.
  ///
  /// In en, this message translates to:
  /// **'The download does not match its expected size.'**
  String get appUpdateTooLarge;

  /// No description provided for @appUpdateSignature.
  ///
  /// In en, this message translates to:
  /// **'The update publisher or signed manifest could not be verified.'**
  String get appUpdateSignature;

  /// No description provided for @appUpdateInvalid.
  ///
  /// In en, this message translates to:
  /// **'The update is invalid or the download is incomplete.'**
  String get appUpdateInvalid;

  /// No description provided for @appUpdateSource.
  ///
  /// In en, this message translates to:
  /// **'The update does not match the candidate approved for this build.'**
  String get appUpdateSource;

  /// No description provided for @appUpdateCompatibility.
  ///
  /// In en, this message translates to:
  /// **'Data compatibility is unknown or unsupported.'**
  String get appUpdateCompatibility;

  /// No description provided for @appUpdateNotNewer.
  ///
  /// In en, this message translates to:
  /// **'This candidate is not newer than the installed application.'**
  String get appUpdateNotNewer;

  /// No description provided for @appUpdateStorage.
  ///
  /// In en, this message translates to:
  /// **'The update could not be safely saved or restored.'**
  String get appUpdateStorage;

  /// No description provided for @appUpdateVersion.
  ///
  /// In en, this message translates to:
  /// **'Candidate {version}'**
  String appUpdateVersion(String version);

  /// No description provided for @appUpdateProgress.
  ///
  /// In en, this message translates to:
  /// **'{downloaded} / {total} bytes'**
  String appUpdateProgress(String downloaded, String total);

  /// No description provided for @browserAutomation.
  ///
  /// In en, this message translates to:
  /// **'Browser automation'**
  String get browserAutomation;

  /// No description provided for @networkCache.
  ///
  /// In en, this message translates to:
  /// **'Network cache'**
  String get networkCache;

  /// No description provided for @remoteScreen.
  ///
  /// In en, this message translates to:
  /// **'Remote screen'**
  String get remoteScreen;

  /// No description provided for @browserOwnerSwitch.
  ///
  /// In en, this message translates to:
  /// **'Browser owner switch'**
  String get browserOwnerSwitch;

  /// No description provided for @browserHostStatus.
  ///
  /// In en, this message translates to:
  /// **'Browser'**
  String get browserHostStatus;

  /// No description provided for @browserExtensionStatus.
  ///
  /// In en, this message translates to:
  /// **'Extension'**
  String get browserExtensionStatus;

  /// No description provided for @browserSessionCount.
  ///
  /// In en, this message translates to:
  /// **'Browser sessions'**
  String get browserSessionCount;

  /// No description provided for @browserSessionScope.
  ///
  /// In en, this message translates to:
  /// **'This list includes the browser host’s sessions. It does not grant control over sessions owned by another agent.'**
  String get browserSessionScope;

  /// No description provided for @browserTabCount.
  ///
  /// In en, this message translates to:
  /// **'Tabs: {count}'**
  String browserTabCount(String count);

  /// No description provided for @browserAllowWork.
  ///
  /// In en, this message translates to:
  /// **'Allow this agent’s browser work'**
  String get browserAllowWork;

  /// No description provided for @cacheConfiguredPort.
  ///
  /// In en, this message translates to:
  /// **'Configured loopback port'**
  String get cacheConfiguredPort;

  /// No description provided for @cacheReachability.
  ///
  /// In en, this message translates to:
  /// **'Cache reachability'**
  String get cacheReachability;

  /// No description provided for @cacheProbeReady.
  ///
  /// In en, this message translates to:
  /// **'Responding'**
  String get cacheProbeReady;

  /// No description provided for @cacheProbeUnavailable.
  ///
  /// In en, this message translates to:
  /// **'Not responding'**
  String get cacheProbeUnavailable;

  /// No description provided for @cacheManagedScope.
  ///
  /// In en, this message translates to:
  /// **'Managed settings are shown separately from reachability. This does not confirm that existing tools use the configured port.'**
  String get cacheManagedScope;

  /// No description provided for @cacheManagedPresent.
  ///
  /// In en, this message translates to:
  /// **'Managed settings present'**
  String get cacheManagedPresent;

  /// No description provided for @cacheManagedAbsent.
  ///
  /// In en, this message translates to:
  /// **'Managed settings not detected'**
  String get cacheManagedAbsent;

  /// No description provided for @cacheAssemblyInvalid.
  ///
  /// In en, this message translates to:
  /// **'Cache configuration is invalid for this app.'**
  String get cacheAssemblyInvalid;

  /// No description provided for @cacheBindTools.
  ///
  /// In en, this message translates to:
  /// **'Use cache for downloads'**
  String get cacheBindTools;

  /// No description provided for @cacheUnbindTools.
  ///
  /// In en, this message translates to:
  /// **'Use direct downloads'**
  String get cacheUnbindTools;

  /// No description provided for @cacheMutationWorking.
  ///
  /// In en, this message translates to:
  /// **'Applying download settings…'**
  String get cacheMutationWorking;

  /// No description provided for @cacheStateUncertain.
  ///
  /// In en, this message translates to:
  /// **'Download settings may have changed. Refresh status before another action.'**
  String get cacheStateUncertain;

  /// No description provided for @cacheCompletionUnconfirmed.
  ///
  /// In en, this message translates to:
  /// **'Could not confirm the changes. Further changes are blocked.'**
  String get cacheCompletionUnconfirmed;

  /// No description provided for @browserSwitchEnabled.
  ///
  /// In en, this message translates to:
  /// **'Enabled'**
  String get browserSwitchEnabled;

  /// No description provided for @browserSwitchDisabled.
  ///
  /// In en, this message translates to:
  /// **'Disabled'**
  String get browserSwitchDisabled;

  /// No description provided for @deviceControl.
  ///
  /// In en, this message translates to:
  /// **'Device control'**
  String get deviceControl;

  /// No description provided for @controlFullauto.
  ///
  /// In en, this message translates to:
  /// **'Full auto'**
  String get controlFullauto;

  /// No description provided for @controlManual.
  ///
  /// In en, this message translates to:
  /// **'Manual'**
  String get controlManual;

  /// No description provided for @controlFullautoHint.
  ///
  /// In en, this message translates to:
  /// **'Trusted agents run supported system tools on this device without per-tool approval. Git writes, remote display and integrations keep their own settings.'**
  String get controlFullautoHint;

  /// No description provided for @controlManualHint.
  ///
  /// In en, this message translates to:
  /// **'Each tool runs only when enabled with its permissions below.'**
  String get controlManualHint;

  /// No description provided for @controlFromServer.
  ///
  /// In en, this message translates to:
  /// **'Server policy, revision {revision}'**
  String controlFromServer(String revision);

  /// No description provided for @controlCached.
  ///
  /// In en, this message translates to:
  /// **'Server unreachable; using policy revision {revision}'**
  String controlCached(String revision);

  /// No description provided for @controlDefaultManual.
  ///
  /// In en, this message translates to:
  /// **'No server policy observed; manual settings apply.'**
  String get controlDefaultManual;

  /// No description provided for @controlSave.
  ///
  /// In en, this message translates to:
  /// **'Save device control'**
  String get controlSave;

  /// No description provided for @controlRecentSignIn.
  ///
  /// In en, this message translates to:
  /// **'Sign in again to switch this device to full auto.'**
  String get controlRecentSignIn;

  /// No description provided for @controlChangedElsewhere.
  ///
  /// In en, this message translates to:
  /// **'The policy changed elsewhere. Refresh and review it.'**
  String get controlChangedElsewhere;

  /// No description provided for @controlUnavailable.
  ///
  /// In en, this message translates to:
  /// **'This server does not support device control yet.'**
  String get controlUnavailable;

  /// No description provided for @purposeServerHosting.
  ///
  /// In en, this message translates to:
  /// **'Server hosting'**
  String get purposeServerHosting;

  /// No description provided for @purposeServerDevDesktop.
  ///
  /// In en, this message translates to:
  /// **'Server development'**
  String get purposeServerDevDesktop;

  /// No description provided for @purposeServerDesktop.
  ///
  /// In en, this message translates to:
  /// **'Server desktop'**
  String get purposeServerDesktop;

  /// No description provided for @purposeBuildDevDesktop.
  ///
  /// In en, this message translates to:
  /// **'Build and development'**
  String get purposeBuildDevDesktop;

  /// No description provided for @purposeDevDesktop.
  ///
  /// In en, this message translates to:
  /// **'Development'**
  String get purposeDevDesktop;

  /// No description provided for @purposeDesktop.
  ///
  /// In en, this message translates to:
  /// **'Desktop'**
  String get purposeDesktop;
}

class _AppLocalizationsDelegate
    extends LocalizationsDelegate<AppLocalizations> {
  const _AppLocalizationsDelegate();

  @override
  Future<AppLocalizations> load(Locale locale) {
    return SynchronousFuture<AppLocalizations>(lookupAppLocalizations(locale));
  }

  @override
  bool isSupported(Locale locale) =>
      <String>['en', 'ru'].contains(locale.languageCode);

  @override
  bool shouldReload(_AppLocalizationsDelegate old) => false;
}

AppLocalizations lookupAppLocalizations(Locale locale) {
  // Lookup logic when only language code is specified.
  switch (locale.languageCode) {
    case 'en':
      return AppLocalizationsEn();
    case 'ru':
      return AppLocalizationsRu();
  }

  throw FlutterError(
    'AppLocalizations.delegate failed to load unsupported locale "$locale". This is likely '
    'an issue with the localizations generation tool. Please file an issue '
    'on GitHub with a reproducible sample app and the gen-l10n configuration '
    'that was used.',
  );
}
