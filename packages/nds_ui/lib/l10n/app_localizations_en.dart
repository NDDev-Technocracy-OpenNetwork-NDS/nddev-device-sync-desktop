// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for English (`en`).
class AppLocalizationsEn extends AppLocalizations {
  AppLocalizationsEn([String locale = 'en']) : super(locale);

  @override
  String get appTitle => 'NDDev OpenNetwork';

  @override
  String get connectTitle => 'Connect to NDS';

  @override
  String get connectHelp =>
      'Enter the HTTPS address of your self-hosted NDS server.';

  @override
  String get serverAddress => 'Server address';

  @override
  String get connect => 'Connect';

  @override
  String get signIn => 'Sign in';

  @override
  String get email => 'Email address';

  @override
  String get sendCode => 'Send sign-in code';

  @override
  String get emailHint =>
      'If this address is permitted, a sign-in code will arrive.';

  @override
  String get code => 'Eight-digit code';

  @override
  String get verify => 'Verify code';

  @override
  String get resend => 'Send another code';

  @override
  String get github => 'Sign in with GitHub';

  @override
  String get githubHelp =>
      'Compare this code with the NDS approval page in your browser. Approve only when both codes match.';

  @override
  String get openGithub => 'Open GitHub';

  @override
  String get awaitApproval => 'Waiting for explicit browser approval';

  @override
  String get cancel => 'Cancel sign-in';

  @override
  String get unavailable => 'Unavailable on this server';

  @override
  String get sessionTitle => 'Your NDS session';

  @override
  String get user => 'User';

  @override
  String get instance => 'Server';

  @override
  String get method => 'Sign-in method';

  @override
  String get expires => 'Expires';

  @override
  String get verified => 'Last verified';

  @override
  String get unverified =>
      'This session has not been revalidated. Check the server connection.';

  @override
  String get refresh => 'Revalidate session';

  @override
  String get logout => 'Revoke session and sign out';

  @override
  String get changeServer => 'Change server';

  @override
  String get diagnostics => 'Diagnostic logs for five minutes';

  @override
  String get diagnosticsOff => 'Stop diagnostic logs';

  @override
  String get privacy =>
      'Session credentials and vault keys stay in native secure storage. The server delivers your vault key after verified sign-in and device enrollment.';

  @override
  String get endpointError =>
      'Use an HTTPS server origin without credentials, path, query or fragment.';

  @override
  String get inputError => 'Check the email address and the eight-digit code.';

  @override
  String get networkError =>
      'The server could not be reached. No fresh session state is confirmed.';

  @override
  String get storeError =>
      'Protected credential storage is unavailable. No plaintext fallback is used.';

  @override
  String get authError =>
      'Sign-in could not be confirmed. Check the code or start again.';

  @override
  String get expiredError => 'This sign-in attempt expired. Start again.';

  @override
  String get rateError => 'Please wait before trying again.';

  @override
  String get cancelledError => 'Sign-in cancelled.';

  @override
  String get responseError =>
      'The server response does not match the NDS contract.';

  @override
  String get personalContextServerRequired =>
      'Update the NDS server before using this version of the client.';

  @override
  String get genericError => 'The operation failed. Try again.';

  @override
  String get launchError => 'The authorization browser could not be opened.';

  @override
  String get working => 'Working…';

  @override
  String validFor(int seconds) {
    String _temp0 = intl.Intl.pluralLogic(
      seconds,
      locale: localeName,
      other: 'Valid for $seconds seconds',
      one: 'Valid for $seconds second',
    );
    return '$_temp0';
  }

  @override
  String retryIn(int seconds) {
    String _temp0 = intl.Intl.pluralLogic(
      seconds,
      locale: localeName,
      other: 'Retry in $seconds seconds',
      one: 'Retry in $seconds second',
    );
    return '$_temp0';
  }

  @override
  String get revalidationRequired =>
      'The credential write may have completed. Revalidate the session before continuing.';

  @override
  String get emailMethod => 'Email code';

  @override
  String get sessionTab => 'NDS session';

  @override
  String get localModules => 'Local modules';

  @override
  String get localReadOnly =>
      'Inspect local tools, history and planned maintenance. Tools retain control of their data.';

  @override
  String get module => 'Module';

  @override
  String get systemInfo => 'System information';

  @override
  String get clipboardHistory => 'Clipboard history';

  @override
  String get cleaner => 'Cleaner';

  @override
  String get updater => 'Updates';

  @override
  String get providerUnavailable => 'The local tool is unavailable.';

  @override
  String get providerFailed =>
      'The local tool could not complete this request.';

  @override
  String get notChecked => 'Not checked';

  @override
  String get reachable => 'Available';

  @override
  String get noRecords => 'No records.';

  @override
  String get nextPage => 'Next page';

  @override
  String get openText => 'Read text';

  @override
  String get cpuUsage => 'CPU usage';

  @override
  String get cpuTemperature => 'CPU temperature';

  @override
  String get memoryUsage => 'Memory used';

  @override
  String get swapUsage => 'Swap used';

  @override
  String get gpuUsage => 'GPU usage';

  @override
  String get gpuMemory => 'GPU memory used';

  @override
  String get gpuTemperature => 'GPU temperature';

  @override
  String get diskRead => 'Disk read rate';

  @override
  String get diskWrite => 'Disk write rate';

  @override
  String get diskTemperature => 'Disk temperature';

  @override
  String get networkReceive => 'Network receive rate';

  @override
  String get networkSend => 'Network send rate';

  @override
  String get errors => 'Errors';

  @override
  String get warnings => 'Warnings';

  @override
  String get checkOk => 'OK';

  @override
  String get checkWarning => 'Warning';

  @override
  String get checkError => 'Error';

  @override
  String get checkInfo => 'Information';

  @override
  String get mode => 'Mode';

  @override
  String get preview => 'Preview';

  @override
  String get executed => 'Executed';

  @override
  String get duration => 'Duration';

  @override
  String get freedBytes => 'Reclaimed space';

  @override
  String get deleted => 'Deleted entries';

  @override
  String get skipped => 'Skipped entries';

  @override
  String get failed => 'Failed entries';

  @override
  String get channel => 'Channel';

  @override
  String get result => 'Result';

  @override
  String get exitClass => 'Exit classification';

  @override
  String get branch => 'Branch';

  @override
  String get upstream => 'Upstream';

  @override
  String get aheadBehind => 'Ahead / behind';

  @override
  String get gdsUnavailable => 'Not available';

  @override
  String get gdsComparisonNotApplicable => 'Not applicable';

  @override
  String get staged => 'Staged changes';

  @override
  String get unstaged => 'Unstaged changes';

  @override
  String get untracked => 'Untracked files';

  @override
  String get conflicts => 'Conflicts';

  @override
  String get remoteFreshness => 'Remote data freshness';

  @override
  String get repositoryPath => 'Absolute repository path';

  @override
  String get lastRun => 'Last run';

  @override
  String get updatePlan => 'Update plan';

  @override
  String get moduleRefresh => 'Refresh';

  @override
  String get moduleCancel => 'Cancel request';

  @override
  String get moduleDiagnostics => 'Diagnostics';

  @override
  String get moduleInputError => 'Check the repository path or selected entry.';

  @override
  String get moduleNetworkError => 'The local tool did not respond in time.';

  @override
  String get moduleResponseError =>
      'The local tool returned an unsupported response.';

  @override
  String get moduleCancelled => 'Request cancelled.';

  @override
  String get moduleShutdownUnconfirmed =>
      'Shutdown could not be confirmed. Access remains closed.';

  @override
  String get missingReading => 'No reading';

  @override
  String get textPreviewLimit => 'Text preview, up to 16 KiB.';

  @override
  String get gdsInstall => 'GDS installation';

  @override
  String get verifiedApplications => 'Verified applications';

  @override
  String get aptObserve => 'APT package observation';

  @override
  String get toolchainObserve => 'Toolchain observation';

  @override
  String get connected => 'Connected';

  @override
  String get connecting => 'Connecting';

  @override
  String get diagnosticsUnavailable =>
      'Local diagnostics are unavailable. Sign-in can continue.';

  @override
  String get localDiagnosticsOnly =>
      'Local diagnostics are initialized. Server delivery status is shown under Clipboard and alerts.';

  @override
  String diagnosticsDegraded(
    String rejected,
    String dropped,
    String failures,
    String pending,
  ) {
    return 'Diagnostics: rejected $rejected, dropped $dropped, write failures $failures, pending $pending.';
  }

  @override
  String get startupFailed =>
      'NDS could not start. Restart the application. If this continues, check the installation and local diagnostics.';

  @override
  String get diagnosticsHttp => 'Network diagnostics';

  @override
  String get diagnosticsIo => 'Local operations';

  @override
  String get devicesTitle => 'Your devices';

  @override
  String get devicesHelp =>
      'Each installation has its own signing key. Active enrolled devices receive access to the vault.';

  @override
  String get devicesUnverified =>
      'Device inventory has not been freshly verified.';

  @override
  String get deviceName => 'Device name';

  @override
  String get enrollDevice => 'Register this device';

  @override
  String get refreshDevices => 'Refresh devices';

  @override
  String get thisDevice => 'This installation';

  @override
  String get deviceActive => 'Active';

  @override
  String get deviceRevoked => 'Revoked';

  @override
  String get deviceCreated => 'Registered';

  @override
  String get deviceRevokedHelp =>
      'This device key has been revoked. It is retained locally and cannot be reused for enrollment.';

  @override
  String get revokeDevice => 'Revoke device';

  @override
  String revokeDeviceHelp(String name) {
    return 'Revoke access for $name? This identity cannot be enrolled again with the same key.';
  }

  @override
  String get keepDevice => 'Keep device';

  @override
  String get vaultTitle => 'Vault and sync';

  @override
  String get vaultHelp =>
      'Your vault opens automatically after sign-in. Changes synchronize through the server; secret values stay in native storage and encrypted records.';

  @override
  String get vaultLocked =>
      'Vault access is unavailable. Verify the session, device and server connection.';

  @override
  String get vaultUnlocked => 'Protected vault key is available.';

  @override
  String vaultCounts(int pending, int conflicts) {
    return 'Pending: $pending · Conflicts: $conflicts';
  }

  @override
  String get vaultKeyIdentity => 'Key identity';

  @override
  String get vaultRecordTitle => 'Record title';

  @override
  String get vaultUsername => 'Username or account label';

  @override
  String get vaultSecret => 'Secret value';

  @override
  String get vaultSave => 'Encrypt and queue for sync';

  @override
  String vaultRevision(String revision) {
    return 'Revision $revision';
  }

  @override
  String get vaultConflict =>
      'Unresolved conflict. Both versions are retained.';

  @override
  String get vaultSync => 'Synchronize now';

  @override
  String get vaultInputError => 'Check the record fields.';

  @override
  String get vaultProofError =>
      'The vault key or record could not be verified.';

  @override
  String get syncedState => 'Synced device and module state';

  @override
  String get publishDeviceState => 'Publish device, session and module state';

  @override
  String get stateObserved => 'Observed at';

  @override
  String get stateChecked => 'Last checked';

  @override
  String get publishModuleState => 'Publish observed module state';

  @override
  String get stateHelp =>
      'These are encrypted observations from signed devices. Their timestamps describe when they were observed, not current reachability. Sync does not execute tools on another device.';

  @override
  String get statePublishError =>
      'State could not be synchronized. Check the session, device and vault connection.';

  @override
  String get stateDevice => 'Device';

  @override
  String get stateDevices => 'Devices';

  @override
  String get vaultRegistrationRequired =>
      'An existing vault key must first be registered by an updated installation that already has it.';

  @override
  String get syncCatchingUp => 'Catching up with the server…';

  @override
  String get syncNotObserved => 'Synchronization has not been observed yet.';

  @override
  String get syncCaughtUp => 'Caught up at the last successful check.';

  @override
  String get passkey => 'Sign in with passkey';

  @override
  String get registerPasskey => 'Create passkey';

  @override
  String get openPasskey => 'Open passkey verification';

  @override
  String get passkeyHelp =>
      'Compare this code in the browser, then verify your passkey. Creating a passkey requires a recent sign-in.';

  @override
  String get utilitiesTitle => 'Clipboard and alerts';

  @override
  String get sharedClipboard => 'Shared clipboard';

  @override
  String get alertsTitle => 'Alerts';

  @override
  String get clipboardHelp =>
      'Text is encrypted and synchronizes automatically with the server. Share only the text you intend to send; sensitive clipboard items and URI data are excluded.';

  @override
  String get sharedText => 'Text to share';

  @override
  String get shareText => 'Share text';

  @override
  String get shareNativeClipboard => 'Share device clipboard';

  @override
  String get clipboardArchive => 'Choose from clipboard archive';

  @override
  String clipboardReady(int sequence, int bytes) {
    return 'Server revision $sequence · $bytes bytes';
  }

  @override
  String get clipboardEmpty => 'No committed shared text yet';

  @override
  String get copySharedClipboard => 'Copy shared text';

  @override
  String utilityError(String code) {
    return 'Operation unavailable: $code';
  }

  @override
  String telemetryDelivery(
    int pending,
    int delivered,
    int dropped,
    int failures,
  ) {
    return 'Queued $pending · Received $delivered · Lost $dropped · Attempts failed $failures';
  }

  @override
  String get telemetryInitialized =>
      'Client delivery is active. Received counts confirm server acceptance.';

  @override
  String get telemetryUnavailable =>
      'Client delivery is unavailable; local diagnostics remain active.';

  @override
  String get noIncidents => 'No incidents';

  @override
  String incidentState(String kind, String state, int occurrences) {
    return '$kind · $state · $occurrences occurrences';
  }

  @override
  String get acknowledgeIncident => 'Acknowledge';

  @override
  String get silenceIncident => 'Silence for one hour';

  @override
  String get moreIncidents => 'Next incidents';

  @override
  String syncLastSeen(String time) {
    return 'Last complete sync: $time';
  }

  @override
  String get vaultDelete => 'Delete record';

  @override
  String vaultDeleteHelp(String title) {
    return 'Delete “$title” on all synchronized devices?';
  }

  @override
  String get vaultResolve => 'Resolve conflict';

  @override
  String vaultResolveHelp(String choice) {
    return 'Apply “$choice”? Other devices will receive this decision after synchronization.';
  }

  @override
  String get operationBusy =>
      'Another operation is in progress or this view changed. Refresh and try again.';

  @override
  String vaultEditing(String title) {
    return 'Editing: $title. Enter the secret value to save.';
  }

  @override
  String get vaultDeleted => 'Deleted record';

  @override
  String vaultConflictingTitle(String title) {
    return 'Competing version: $title';
  }

  @override
  String get vaultConflictingDelete =>
      'The competing version deletes this record.';

  @override
  String get vaultKeepServer => 'Keep server version';

  @override
  String get vaultUseConflict => 'Use competing version';

  @override
  String get vaultCopySecret => 'Copy secret';

  @override
  String get vaultEdit => 'Edit';

  @override
  String get vaultNextPage => 'Next records';

  @override
  String get vaultFirstPage => 'First records';

  @override
  String get passkeysTitle => 'Your passkeys';

  @override
  String get passkeysHelp =>
      'Removing a passkey ends sessions created with it. Removal requires signing in within the last five minutes.';

  @override
  String get passkeysEmpty => 'No passkeys registered.';

  @override
  String passkeyCreated(String time) {
    return 'Created: $time';
  }

  @override
  String get passkeyRemove => 'Remove passkey';

  @override
  String passkeyRemoveHelp(String id) {
    return 'Remove passkey $id? Sessions created with this key will be revoked, possibly including this session.';
  }

  @override
  String get passkeyFreshLogin => 'Sign in again before changing passkeys.';

  @override
  String get keepPasskey => 'Keep passkey';

  @override
  String get passkeyMethod => 'Passkey';

  @override
  String get actionCancel => 'Cancel';

  @override
  String get exportUnknown => 'Server export policy has not been confirmed.';

  @override
  String get exportEnabled => 'Server export is enabled.';

  @override
  String get exportDisabled => 'The operator disabled server export.';

  @override
  String get exportPaused => 'Delivery is paused. Return to the app to resume.';

  @override
  String get eventsLabel => 'Events';

  @override
  String get spansLabel => 'Operation traces';

  @override
  String exportLastSeen(String time) {
    return 'Last server acceptance: $time';
  }

  @override
  String incidentsObserved(String time) {
    return 'Alert state observed: $time';
  }

  @override
  String get incidentsUnavailable =>
      'Alert state could not be refreshed. Displayed entries may be outdated.';

  @override
  String get clipboardUnavailable =>
      'Shared clipboard state could not be refreshed.';

  @override
  String get clipboardRejected =>
      'This clipboard item is private, unsupported or too large to share. Choose plain text up to 8 KiB.';

  @override
  String get clipboardCopied => 'Copied to device clipboard.';

  @override
  String get offlineAuthorized =>
      'Offline access uses the last verified session and device. Changes remain on this device until synchronization.';

  @override
  String get storageFull => 'Local storage is full. The change was not saved.';

  @override
  String get logoutPending =>
      'Signed out locally. Server sign-out will complete after reconnecting.';

  @override
  String get recentAuthenticationRequired =>
      'Sign in again to manage passkeys. Your current session remains valid.';

  @override
  String get browserPrivacy =>
      'The session stays in a protected browser cookie. Vault keys are encrypted in this browser profile; the server delivers them after verified sign-in and device enrollment.';

  @override
  String get browserVaultHelp =>
      'Your vault opens automatically after sign-in. Changes are stored encrypted in this browser and synchronize through the server. Keep this browser’s site data until all changes are synchronized.';

  @override
  String get browserStoragePersistent =>
      'The browser granted persistent storage. Clearing site data still removes local changes.';

  @override
  String get browserStorageEvictable =>
      'The browser may remove site data under storage pressure. Unsynchronized changes would be lost.';

  @override
  String get vaultRecordPending => 'Local change awaiting synchronization';

  @override
  String get telemetryPersistenceReady =>
      'Browser log storage has confirmed writes.';

  @override
  String get telemetryPersistenceUnavailable =>
      'Durable browser log storage has not confirmed a successful write. Unstored events can be lost when this page closes.';

  @override
  String get vaultStorageUnavailable =>
      'Local encrypted storage is unavailable. Check this browser or device’s storage permissions and support.';

  @override
  String get vaultDependencyUnavailable =>
      'The vault could not access required local storage or server services.';

  @override
  String get serverCapacityExhausted =>
      'Server storage capacity is exhausted. Your encrypted changes remain on this device. Automatic sending is paused while NDS checks for a safe retry; you can also retry manually.';

  @override
  String get catalogTitle => 'GitHub repositories';

  @override
  String get catalogHelp =>
      'Choose a vault record containing a GitHub access token to update the saved catalog. Each request reads one page available to that credential.';

  @override
  String get catalogCredential => 'Vault credential';

  @override
  String get catalogRefreshCredentials => 'Refresh vault records';

  @override
  String get catalogMoreCredentials => 'Next vault records';

  @override
  String get catalogNoCredentials =>
      'No selectable records on this vault page. Open the vault to add or resolve a credential record.';

  @override
  String get catalogRead => 'Read first page';

  @override
  String get catalogNextPage => 'Next repository page';

  @override
  String get catalogChooseCredential => 'Choose a current vault record.';

  @override
  String get catalogSignIn =>
      'Sign in and verify this device to read repositories.';

  @override
  String get catalogCredentialChanged =>
      'This vault record changed. Refresh records and select it again.';

  @override
  String get catalogInvalidResponse => 'Repository data could not be verified.';

  @override
  String get catalogProviderFailed =>
      'GitHub could not return this page. Check the credential’s access and try again.';

  @override
  String get catalogUnavailable => 'Repository catalog is unavailable.';

  @override
  String get catalogObservationUnknown => 'Observation time is unavailable.';

  @override
  String catalogObserved(String time) {
    return 'Observed: $time';
  }

  @override
  String catalogPrincipal(String login) {
    return 'GitHub account: $login';
  }

  @override
  String get catalogPageScope =>
      'Only repositories available to the selected credential are shown. Each request replaces this page.';

  @override
  String get catalogEmptyPage => 'No repositories on this page.';

  @override
  String get catalogOrganization => 'Organization';

  @override
  String get catalogPersonal => 'Personal account';

  @override
  String get catalogPublic => 'Public';

  @override
  String get catalogPrivate => 'Private';

  @override
  String get catalogInternal => 'Internal';

  @override
  String get catalogArchived => 'Archived';

  @override
  String get catalogHasMore =>
      'More repositories are available in the next page.';

  @override
  String get catalogAccessEnd =>
      'End of the list available to this credential. Other access scopes may include more repositories.';

  @override
  String get catalogLimitReached =>
      'Local page limit reached. GitHub still reports more repositories; this listing is incomplete.';

  @override
  String get catalogCancelled => 'Repository request cancelled.';

  @override
  String get catalogTimeout => 'Repository request timed out. Try again.';

  @override
  String get catalogPaused =>
      'Repository requests are paused while this app is in the background.';

  @override
  String get screenView => 'View screen';

  @override
  String get screenEnded => 'Screen viewing ended.';

  @override
  String get screenOpening => 'Waiting for screen access…';

  @override
  String get screenUnavailable => 'The screen is unavailable.';

  @override
  String get screenStop => 'Stop viewing';

  @override
  String get screenRequests => 'Screen requests';

  @override
  String get screenRequestsHelp =>
      'Refresh requests from devices allowed by your current company policy. Sharing starts only after you allow this request.';

  @override
  String get screenConsent =>
      'Allow this device to view your screen for the requested session?';

  @override
  String get screenAllow => 'Allow screen viewing';

  @override
  String get screenAccessAllowed =>
      'Screen access is allowed for this session.';

  @override
  String get screenStopSharing => 'Stop sharing';

  @override
  String get catalogSaved => 'Saved catalog';

  @override
  String get catalogProvider => 'Update from GitHub';

  @override
  String get catalogSavedHelp =>
      'Encrypted observations retained in NDS. Dates show when data was observed, not current GitHub access or freshness.';

  @override
  String get catalogReadSaved => 'Refresh saved page';

  @override
  String get catalogSavedNext => 'Next saved page';

  @override
  String get catalogReadSavedAgain =>
      'Refresh the saved catalog and try again.';

  @override
  String catalogMissingOwner(String id) {
    return 'Owner $id: details have not synchronized yet.';
  }

  @override
  String get catalogCapacityReached =>
      'The catalog could not finish updating because synchronization storage reached its limit. Saved observations remain available.';

  @override
  String get companySpace => 'Space';

  @override
  String get companyPersonal => 'Personal space';

  @override
  String get companyPersonalHelp =>
      'Organization membership does not change your personal storage owner. Select an organization to view its current membership.';

  @override
  String get companyName => 'Organization name';

  @override
  String get companyCreate => 'Create organization';

  @override
  String get companyCreateHelp =>
      'This action uses your personal identity and creates a separate organization with you as its first administrator.';

  @override
  String get companyReviewedUnknown =>
      'Current authorized state was reviewed. Finishing the retained command does not undo any previously submitted change.';

  @override
  String get companyServerRequired =>
      'This server does not support explicit organization contexts. No other company is selected automatically.';

  @override
  String get companyContextClosed =>
      'This context is closed. Refresh memberships or sign in again before another action.';

  @override
  String get companyTitle => 'Organizations';

  @override
  String get companyPrivacy =>
      'Organization administration is online. Your personal Vault, secrets and repositories keep their original owner.';

  @override
  String get companyAdmin => 'Administrator';

  @override
  String get companyUser => 'User';

  @override
  String get companyCompleted =>
      'The server confirmed the change. Refresh to see the current state.';

  @override
  String get companyLastAdmin =>
      'The last active administrator cannot be removed or demoted.';

  @override
  String get companyConflict =>
      'The member changed since this page was loaded. Review the current state.';

  @override
  String get companyExpired =>
      'The command has expired. Review the current state before a new action.';

  @override
  String get companyInvitationUnavailable =>
      'This invitation or recipient is no longer available for this action.';

  @override
  String get companyDenied =>
      'Your current membership does not allow this action.';

  @override
  String get companyInvalid => 'Check the email address and selected action.';

  @override
  String get companyPending =>
      'The original command is retained for retry. If the connection failed, the server may have applied it. Review the current state before creating another command. Closing this screen does not resend it.';

  @override
  String get companyRetry => 'Retry the same command';

  @override
  String get companyReview => 'Review state and finish this command';

  @override
  String get companyMembers => 'Members';

  @override
  String get companyActive => 'Active';

  @override
  String get companyRemoved => 'Removed';

  @override
  String get companyRemove => 'Remove member';

  @override
  String get companyChangeRole => 'Change role';

  @override
  String companyChangeHelp(String member) {
    return 'Apply this membership change to $member? Removing membership closes organization access; personal sessions and data remain separate.';
  }

  @override
  String get companyConfirm => 'Confirm';

  @override
  String get companyNextMembers => 'Next members';

  @override
  String get companyInvite => 'Invite by email';

  @override
  String get companyDeliveryHelp =>
      'Sending an invitation, acceptance by the mail service and joining the company are separate steps. Links expire after seven days.';

  @override
  String get companyInvitePending => 'Awaiting acceptance';

  @override
  String get companyJoined => 'Joined';

  @override
  String get companyInviteExpired => 'Expired';

  @override
  String get companyInviteRevoked => 'Revoked';

  @override
  String get companyDeliveryPending => 'Mail: awaiting delivery result';

  @override
  String get companyDeliveryAccepted => 'Mail: accepted by the mail service';

  @override
  String get companyDeliveryFailed => 'Mail: not sent';

  @override
  String get companyDeliveryUnknown => 'Mail: delivery result unknown';

  @override
  String get companyRevoke => 'Revoke invitation';

  @override
  String get companyNextInvitations => 'Next invitations';

  @override
  String get invitationImport => 'Open invitation';

  @override
  String get invitationLink => 'Invitation link';

  @override
  String get invitationVerify =>
      'Invitation loaded. Enter the invited email address and verify the code sent to it. The link alone does not sign you in.';

  @override
  String get invitationSignOut =>
      'An invitation is waiting. Sign out first to verify the invited email address.';

  @override
  String get passkeyAccountHint =>
      'For passkey sign-in, enter the email address linked to your NDS account. With multiple members, an account must be selected.';

  @override
  String get screenTargets => 'Devices for screen viewing';

  @override
  String get screenTargetsEmpty => 'No active devices.';

  @override
  String get screenNotRegistered => 'Open screen sharing on this device first.';

  @override
  String get screenTargetsFailed =>
      'Could not load devices. Refresh to check access and connection.';

  @override
  String get moduleManagementHelp =>
      'Enable modules and choose their NDS permissions for this device. System or browser consent is still required when a feature uses it.';

  @override
  String get moduleEnabled => 'Module enabled';

  @override
  String get modulePermissions => 'NDS permissions';

  @override
  String get moduleSave => 'Save module settings';

  @override
  String get moduleSaved =>
      'Settings saved on this device; synchronization continues separately.';

  @override
  String get moduleDisabled => 'Enable this module to use its features.';

  @override
  String get modulePermissionRequired =>
      'Choose the required NDS permissions before using this feature.';

  @override
  String get moduleConfigurationConflict =>
      'Module settings conflict. No permission is granted until the conflict is resolved.';

  @override
  String get moduleDeviceRequired =>
      'Sign in and register this device to manage modules.';

  @override
  String get moduleNotInstalled =>
      'The required local tool is not installed or has not been found.';

  @override
  String get moduleInstalled => 'Available on this device';

  @override
  String get moduleInstallationUnknown => 'Availability has not been checked';

  @override
  String get moduleMetadataError =>
      'Module settings could not be refreshed. The last operation result is unchanged.';

  @override
  String get permissionReadOnly => 'Read local information';

  @override
  String get permissionCredentialStore => 'Use protected credential references';

  @override
  String get permissionProcessExecution => 'Run the declared local tool';

  @override
  String get permissionLocalNetwork => 'Communicate with the declared service';

  @override
  String get permissionUserService => 'Use the current user service';

  @override
  String get permissionClipboard => 'Use clipboard features';

  @override
  String get moduleSettingsChanged =>
      'Module settings changed. Refresh the form before saving your choice.';

  @override
  String get moduleReloadDraft => 'Reload settings';

  @override
  String get catalogSourceHelp =>
      'This choice is saved and synchronized across your account. Enable GDS permissions separately on each device.';

  @override
  String get catalogClearSource => 'Clear GitHub choice';

  @override
  String get moduleStateQueued => 'Local states queued for synchronization.';

  @override
  String get permissionGitWrite =>
      'Create checkouts and synchronize existing Git commits';

  @override
  String get permissionBrowserControl => 'Control agent browser tabs';

  @override
  String get browserControlScope =>
      'Allows agents to automate tabs in NDS groups. Other browser tabs remain outside this permission.';

  @override
  String get repositoryConnect => 'Connect checkout';

  @override
  String repositoryConnected(String path) {
    return 'Checkout connected: $path';
  }

  @override
  String repositoryRegistrationPending(String path) {
    return 'Checkout exists at $path. Retry connection to finish NDS registration.';
  }

  @override
  String get repositoryIntentQueued =>
      'Connection intent was queued for NDS synchronization.';

  @override
  String get repositoryPathOccupied =>
      'This location is already occupied. Existing files were preserved.';

  @override
  String get repositoryDifferentInstallation =>
      'This checkout belongs to another NDS account or installation.';

  @override
  String get repositoryRelocationRequired =>
      'The saved connection uses another location. Review its move before connecting here.';

  @override
  String get repositoryWrongRemote =>
      'The Git origin or stored repository identity does not match the selected repository.';

  @override
  String get repositoryUnsupported =>
      'Local checkout connection is not supported on this device.';

  @override
  String get repositorySyncRunning => 'Synchronizing existing commits…';

  @override
  String get repositorySyncCurrent => 'Commits are up to date';

  @override
  String get repositorySyncPublished => 'Existing commits published';

  @override
  String get repositorySyncFastForwarded => 'Checkout fast-forwarded';

  @override
  String get repositorySyncDirty =>
      'Uncommitted changes preserved; synchronization paused for this checkout';

  @override
  String get repositorySyncDiverged => 'Histories diverged; changes preserved';

  @override
  String get repositorySyncRefused =>
      'Synchronization refused; existing work preserved';

  @override
  String get appUpdateTitle => 'NDS application updates';

  @override
  String get appUpdateCheck => 'Check for updates';

  @override
  String get appUpdateDownload => 'Download update';

  @override
  String get appUpdateCancel => 'Cancel download';

  @override
  String get appUpdateIdle => 'Check for an application update.';

  @override
  String get appUpdateChecking => 'Checking the update…';

  @override
  String get appUpdateAvailable =>
      'A verified update is available to download.';

  @override
  String get appUpdateDownloading => 'Downloading the update…';

  @override
  String get appUpdateStaged =>
      'The download is verified and saved. Installation is not available in this build.';

  @override
  String get appUpdateUnsupported =>
      'Application updates are not supported on this platform.';

  @override
  String get appUpdateNotConfigured =>
      'Application updates are not configured for this build.';

  @override
  String get appUpdateBusy =>
      'Another update operation is running. Try again when it finishes.';

  @override
  String get appUpdateCancelled => 'The download was cancelled.';

  @override
  String get appUpdateTimeout => 'The update operation timed out. Try again.';

  @override
  String get appUpdateNetwork =>
      'The update could not be retrieved from an approved source.';

  @override
  String get appUpdateTooLarge =>
      'The download does not match its expected size.';

  @override
  String get appUpdateSignature =>
      'The update publisher or signed manifest could not be verified.';

  @override
  String get appUpdateInvalid =>
      'The update is invalid or the download is incomplete.';

  @override
  String get appUpdateSource =>
      'The update does not match the candidate approved for this build.';

  @override
  String get appUpdateCompatibility =>
      'Data compatibility is unknown or unsupported.';

  @override
  String get appUpdateNotNewer =>
      'This candidate is not newer than the installed application.';

  @override
  String get appUpdateStorage =>
      'The update could not be safely saved or restored.';

  @override
  String appUpdateVersion(String version) {
    return 'Candidate $version';
  }

  @override
  String appUpdateProgress(String downloaded, String total) {
    return '$downloaded / $total bytes';
  }

  @override
  String get browserAutomation => 'Browser automation';

  @override
  String get networkCache => 'Network cache';

  @override
  String get browserOwnerSwitch => 'Browser owner switch';

  @override
  String get browserHostStatus => 'Browser';

  @override
  String get browserExtensionStatus => 'Extension';

  @override
  String get browserSessionCount => 'Browser sessions';

  @override
  String get browserSessionScope =>
      'This list includes the browser host’s sessions. It does not grant control over sessions owned by another agent.';

  @override
  String browserTabCount(String count) {
    return 'Tabs: $count';
  }

  @override
  String get browserAllowWork => 'Allow this agent’s browser work';

  @override
  String get cacheConfiguredPort => 'Configured loopback port';

  @override
  String get cacheReachability => 'Cache reachability';

  @override
  String get cacheProbeReady => 'Responding';

  @override
  String get cacheProbeUnavailable => 'Not responding';

  @override
  String get cacheManagedScope =>
      'Managed settings are shown separately from reachability. This does not confirm that existing tools use the configured port.';

  @override
  String get cacheManagedPresent => 'Managed settings present';

  @override
  String get cacheManagedAbsent => 'Managed settings not detected';

  @override
  String get cacheAssemblyInvalid =>
      'Cache configuration is invalid for this app.';

  @override
  String get cacheBindTools => 'Use cache for downloads';

  @override
  String get cacheUnbindTools => 'Use direct downloads';

  @override
  String get cacheMutationWorking => 'Applying download settings…';

  @override
  String get cacheStateUncertain =>
      'Download settings may have changed. Refresh status before another action.';

  @override
  String get cacheCompletionUnconfirmed =>
      'Could not confirm the changes. Further changes are blocked.';

  @override
  String get browserSwitchEnabled => 'Enabled';

  @override
  String get browserSwitchDisabled => 'Disabled';
}
