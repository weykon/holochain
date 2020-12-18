use crate::core::ribosome::error::RibosomeResult;
use crate::core::ribosome::CallContext;
use crate::core::ribosome::RibosomeT;
use holochain_keystore::keystore_actor::KeystoreSenderExt;
use holochain_zome_types::X25519XSalsa20Poly1305EncryptInput;
use holochain_zome_types::X25519XSalsa20Poly1305EncryptOutput;
use std::sync::Arc;

pub fn x_25519_x_salsa20_poly1305_encrypt(
    _ribosome: Arc<impl RibosomeT>,
    call_context: Arc<CallContext>,
    input: X25519XSalsa20Poly1305EncryptInput,
) -> RibosomeResult<X25519XSalsa20Poly1305EncryptOutput> {
    Ok(X25519XSalsa20Poly1305EncryptOutput::new(
        tokio_safe_block_on::tokio_safe_block_forever_on(async move {
            call_context
                .host_access
                .keystore()
                .x_25519_x_salsa20_poly1305_encrypt(input.into_inner())
                .await
        })?,
    ))
}

#[cfg(test)]
#[cfg(feature = "slow_tests")]
pub mod wasm_test {

    use crate::fixt::ZomeCallHostAccessFixturator;
    use ::fixt::prelude::*;
    use hdk3::prelude::*;
    use holochain_wasm_test_utils::TestWasm;

    #[tokio::test(threaded_scheduler)]
    async fn invoke_import_x_25519_x_salsa20_poly1305_encrypt_test() {
        let test_env = holochain_state::test_utils::test_cell_env();
        let env = test_env.env();
        let mut workspace =
            crate::core::workflow::CallZomeWorkspace::new(env.clone().into()).unwrap();
        crate::core::workflow::fake_genesis(&mut workspace.source_chain)
            .await
            .unwrap();

        let workspace_lock = crate::core::workflow::CallZomeWorkspaceLock::new(workspace);

        let mut host_access = fixt!(ZomeCallHostAccess);
        host_access.workspace = workspace_lock;
        let alice: X25519PubKey = {
            let output: CreateX25519KeypairOutput = crate::call_test_ribosome!(
                host_access,
                TestWasm::XSalsa20Poly1305,
                "create_x25519_keypair",
                ()
            );
            output.into_inner()
        };
        assert_eq!(
            &alice.as_ref(),
            &[
                65, 17, 71, 31, 48, 10, 48, 208, 3, 220, 71, 246, 83, 246, 74, 221, 3, 123, 54, 48,
                160, 192, 179, 207, 115, 6, 19, 53, 233, 231, 167, 75,
            ]
        );
        let bob: X25519PubKey = {
            let output: CreateX25519KeypairOutput = crate::call_test_ribosome!(
                host_access,
                TestWasm::XSalsa20Poly1305,
                "create_x25519_keypair",
                ()
            );
            output.into_inner()
        };
        assert_eq!(
            &bob.as_ref(),
            &[
                139, 250, 5, 51, 172, 9, 244, 251, 44, 226, 178, 145, 1, 252, 128, 237, 27, 225,
                11, 171, 153, 205, 115, 228, 72, 211, 110, 41, 115, 48, 251, 98
            ],
        );
        let carol: X25519PubKey = {
            let output: CreateX25519KeypairOutput = crate::call_test_ribosome!(
                host_access,
                TestWasm::XSalsa20Poly1305,
                "create_x25519_keypair",
                ()
            );
            output.into_inner()
        };
        assert_eq!(
            &carol.as_ref(),
            &[
                211, 158, 23, 148, 162, 67, 112, 72, 185, 58, 136, 103, 76, 164, 39, 200, 83, 124,
                57, 64, 234, 36, 102, 209, 80, 32, 77, 68, 108, 242, 71, 41
            ],
        );

        let data = XSalsa20Poly1305Data::from(vec![1, 2, 3, 4]);

        let encrypt_input = X25519XSalsa20Poly1305EncryptInput::new(X25519XSalsa20Poly1305Encrypt::new(
            alice.clone(),
            bob.clone(),
            data.clone(),
        ));

        let encrypt_output: X25519XSalsa20Poly1305EncryptOutput = crate::call_test_ribosome!(
            host_access,
            TestWasm::XSalsa20Poly1305,
            "x_25519_x_salsa20_poly1305_encrypt",
            encrypt_input
        );

        let decrypt_input = X25519XSalsa20Poly1305DecryptInput::new(holochain_zome_types::x_salsa20_poly1305::X25519XSalsa20Poly1305Decrypt::new(bob.clone(), alice.clone(), encrypt_output.clone().into_inner()));
        let decrypt_output: X25519XSalsa20Poly1305DecryptOutput = crate::call_test_ribosome!(
            host_access,
            TestWasm::XSalsa20Poly1305,
            "x_25519_x_salsa20_poly1305_decrypt",
            decrypt_input
        );

        assert_eq!(
            decrypt_output.into_inner(),
            Some(data.clone()),
        );

        let bad_decrypt_input = X25519XSalsa20Poly1305DecryptInput::new(holochain_zome_types::x_salsa20_poly1305::X25519XSalsa20Poly1305Decrypt::new(carol.clone(), alice.clone(), encrypt_output.into_inner()));
        let bad_decrypt_output: X25519XSalsa20Poly1305DecryptOutput = crate::call_test_ribosome!(
            host_access,
            TestWasm::XSalsa20Poly1305,
            "x_25519_x_salsa20_poly1305_decrypt",
            bad_decrypt_input
        );

        assert_eq!(
            bad_decrypt_output.into_inner(),
            None,
        );

    }
}