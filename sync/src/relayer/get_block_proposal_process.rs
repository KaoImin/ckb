use crate::relayer::Relayer;
use ckb_network::{CKBProtocolContext, PeerIndex};
use ckb_protocol::{cast, GetBlockProposal, RelayMessage};
use ckb_shared::index::ChainIndex;
use ckb_util::TryInto;
use failure::Error as FailureError;
use flatbuffers::FlatBufferBuilder;

pub struct GetBlockProposalProcess<'a, CI: ChainIndex + 'a> {
    message: &'a GetBlockProposal<'a>,
    relayer: &'a Relayer<CI>,
    peer: PeerIndex,
    nc: &'a mut CKBProtocolContext,
}

impl<'a, CI> GetBlockProposalProcess<'a, CI>
where
    CI: ChainIndex + 'static,
{
    pub fn new(
        message: &'a GetBlockProposal,
        relayer: &'a Relayer<CI>,
        peer: PeerIndex,
        nc: &'a mut CKBProtocolContext,
    ) -> Self {
        GetBlockProposalProcess {
            message,
            nc,
            relayer,
            peer,
        }
    }

    pub fn execute(self) -> Result<(), FailureError> {
        let mut pending_proposals_request = self.relayer.state.pending_proposals_request.lock();
        let proposal_transactions = cast!(self.message.proposal_transactions())?;

        let transactions = {
            let chain_state = self.relayer.shared.chain_state().lock();
            let tx_pool = chain_state.tx_pool();

            let proposals = proposal_transactions
                .iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<_>, FailureError>>();

            proposals?
                .into_iter()
                .filter_map(|short_id| {
                    tx_pool.get_tx(&short_id).or({
                        pending_proposals_request
                            .entry(short_id)
                            .or_insert_with(Default::default)
                            .insert(self.peer);
                        None
                    })
                })
                .collect::<Vec<_>>()
        };

        let fbb = &mut FlatBufferBuilder::new();
        let message = RelayMessage::build_block_proposal(fbb, &transactions);
        fbb.finish(message, None);

        let _ = self.nc.send(self.peer, fbb.finished_data().to_vec());
        Ok(())
    }
}
