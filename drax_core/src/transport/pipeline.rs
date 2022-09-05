use crate::transport::TransportProcessorContext;
use std::mem::{ManuallyDrop, MaybeUninit};

pub trait ChainProcessor {
    type Input;
    type Output;

    fn process(
        &mut self,
        context: &mut TransportProcessorContext,
        input: Self::Input,
    ) -> super::Result<Self::Output>;
}

pub fn link<T1, T2, T3>(
    linkage: BoxedChain<T1, T2>,
    function: BoxedChain<T2, T3>,
) -> ProcessChainLink<T1, T2, T3> {
    ProcessChainLink {
        process_chain_linkage: linkage,
        process_chain_fn: function,
    }
}

pub type BoxedChain<T1, T2> = Box<dyn ChainProcessor<Input = T1, Output = T2>>;

pub struct ProcessChainLink<T1, T2, T3> {
    process_chain_linkage: BoxedChain<T1, T2>,
    process_chain_fn: BoxedChain<T2, T3>,
}

impl<T1, T2, T3> ProcessChainLink<T1, T2, T3> {
    pub fn into_outer(self) -> (BoxedChain<T1, T2>, BoxedChain<T2, T3>) {
        (self.process_chain_linkage, self.process_chain_fn)
    }
}

impl<T1, T2, T3> ChainProcessor for ProcessChainLink<T1, T2, T3> {
    type Input = T1;
    type Output = T3;

    fn process(
        &mut self,
        context: &mut TransportProcessorContext,
        input: Self::Input,
    ) -> super::Result<Self::Output> {
        let linkage = self.process_chain_linkage.process(context, input)?;
        self.process_chain_fn.process(context, linkage)
    }
}
