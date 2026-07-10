pub(super) const SET_PAGE: &str = r#"<!doctype html><title>loading</title><style>body{font:24px sans-serif;color:#111;background:#fff}</style><body>Profile persistence</body><script>localStorage.setItem('ely_storage','persisted');const cookie=document.cookie.includes('ely_cookie=persisted')?'yes':'no';const storage=localStorage.getItem('ely_storage')==='persisted'?'yes':'no';document.title=`stored-cookie-${cookie}-storage-${storage}`;</script>"#;

pub(super) const READ_PAGE: &str = r#"<!doctype html><title>loading</title><style>body{font:24px sans-serif;color:#111;background:#fff}</style><body>Profile persistence</body><script>const cookie=document.cookie.includes('ely_cookie=persisted')?'yes':'no';const storage=localStorage.getItem('ely_storage')==='persisted'?'yes':'no';document.title=`read-cookie-${cookie}-storage-${storage}`;</script>"#;

pub(super) const HISTORY_PAGE: &str = r#"<!doctype html><title>loading</title><style>body{font:24px sans-serif;color:#111;background:#fff}</style><body>History mutation</body><script>history.replaceState({},'', '/history?state=1');document.title='history-ready';</script>"#;

pub(super) const OVERSIZED_HISTORY_PAGE: &str = r#"<!doctype html><title>oversized-history</title><script>history.replaceState({},'', '/oversized?' + 'a'.repeat(32768));</script>"#;

pub(super) const WHITE_PAGE: &str = r#"<!doctype html><title>white-ready</title><style>html,body{margin:0;width:100%;height:100%;background:#fff}</style>"#;

pub(super) const RSA_PRIVATE_OPERATION_PAGE: &str = r#"<!doctype html><title>rsa-private-operations-loading</title><body>RSA private-operation gate</body><script>
(async () => {
  const subtle = crypto.subtle;
  const data = new TextEncoder().encode('ELY');
  const digest = await subtle.digest('SHA-256', data);
  if (digest.byteLength !== 32) throw new Error('digest-failed');

  const loadKey = async path => {
    const response = await fetch(path);
    if (!response.ok) throw new Error(`key-fetch-${response.status}`);
    return response.arrayBuffer();
  };
  const [privateKeyData, publicKeyData] = await Promise.all([
    loadKey('/rsa-private.der'),
    loadKey('/rsa-public.der'),
  ]);
  const expectOperationError = async (label, operation) => {
    try {
      await operation();
      throw new Error(`${label}-succeeded`);
    } catch (error) {
      if (error.name === 'OperationError') return;
      throw error;
    }
  };

  const oaep = { name: 'RSA-OAEP', hash: 'SHA-256' };
  const oaepPrivate = await subtle.importKey(
    'pkcs8', privateKeyData, oaep, false, ['decrypt', 'unwrapKey']);
  const oaepPublic = await subtle.importKey(
    'spki', publicKeyData, oaep, false, ['encrypt', 'wrapKey']);
  const ciphertext = await subtle.encrypt({ name: 'RSA-OAEP' }, oaepPublic, data);
  await expectOperationError('oaep-decrypt', () =>
    subtle.decrypt({ name: 'RSA-OAEP' }, oaepPrivate, ciphertext));
  const wrappingTarget = await subtle.importKey(
    'raw', new Uint8Array(16), { name: 'AES-GCM' }, true, ['encrypt']);
  const wrapped = await subtle.wrapKey(
    'raw', wrappingTarget, oaepPublic, { name: 'RSA-OAEP' });
  await expectOperationError('oaep-unwrap', () =>
    subtle.unwrapKey(
      'raw', wrapped, oaepPrivate, { name: 'RSA-OAEP' },
      { name: 'AES-GCM' }, false, ['encrypt']));

  const pss = { name: 'RSA-PSS', hash: 'SHA-256' };
  const pssPrivate = await subtle.importKey('pkcs8', privateKeyData, pss, false, ['sign']);
  await expectOperationError('pss-sign', () =>
    subtle.sign({ name: 'RSA-PSS', saltLength: 32 }, pssPrivate, data));

  const pkcs = { name: 'RSASSA-PKCS1-v1_5', hash: 'SHA-256' };
  const pkcsPrivate = await subtle.importKey('pkcs8', privateKeyData, pkcs, false, ['sign']);
  await expectOperationError('pkcs-sign', () =>
    subtle.sign({ name: 'RSASSA-PKCS1-v1_5' }, pkcsPrivate, data));
  const pkcsPublic = await subtle.importKey('spki', publicKeyData, pkcs, false, ['verify']);
  const verified = await subtle.verify(
    { name: 'RSASSA-PKCS1-v1_5' }, pkcsPublic, new Uint8Array(256), data);
  if (verified) throw new Error('pkcs-invalid-signature-verified');

  document.title = 'rsa-private-operations-gated';
})().catch(error => {
  document.title = `rsa-private-operations-failed-${error.name}`;
});
</script>"#;

pub(super) const RSA_PRIVATE_KEY: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../third_party/rsa/tests/examples/pkcs8/rsa2048-priv.der"
));
pub(super) const RSA_PUBLIC_KEY: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../third_party/rsa/tests/examples/pkcs8/rsa2048-pub.der"
));
