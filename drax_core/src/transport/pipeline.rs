use crate::transport::TransportProcessorContext;
use std::sync::Arc;

macro_rules! process_chain_link_internal {
    ($t1:ident, $t2:ident) => {
        type Input = $t1;
        type Output = $t2;

        fn process(
            &self,
            context: &mut TransportProcessorContext,
            input: Self::Input,
        ) -> super::Result<Self::Output> {
            let linkage = self.process_chain_linkage.process(context, input)?;
            self.process_chain_fn.process(context, linkage)
        }
    };
}

pub trait ChainProcessor {
    type Input;
    type Output;

    fn process<'a>(
        &'a self,
        context: &'a mut TransportProcessorContext,
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
    process_chain_link_internal!(T1, T3);
}

pub type ShareChain<T1, T2> = Arc<dyn ChainProcessor<Input = T1, Output = T2> + Send + Sync>;

pub struct ShareChainLink<T1: Send + Sync, T2: Send + Sync, T3: Send + Sync> {
    process_chain_linkage: ShareChain<T1, T2>,
    process_chain_fn: ShareChain<T2, T3>,
}

impl<T1: Send + Sync, T2: Send + Sync, T3: Send + Sync> ShareChainLink<T1, T2, T3> {
    pub fn into_outer(self) -> (ShareChain<T1, T2>, ShareChain<T2, T3>) {
        (self.process_chain_linkage, self.process_chain_fn)
    }
}

impl<T1: Send + Sync, T2: Send + Sync, T3: Send + Sync> ChainProcessor
    for ShareChainLink<T1, T2, T3>
{
    process_chain_link_internal!(T1, T3);
}

#[macro_export]
macro_rules! link {
    ($l1:expr, $l2:expr) => {
        drax::transport::pipeline::link(Box::new($l1), Box::new($l2));
    };
    ($l1:expr, $l2:expr, $($etc:expr)+) => {
        link!($l1, link!($l2, $($etc)+));
    };
}

#[macro_export]
macro_rules! share_link {
    ($l1:expr, $l2:expr) => {
        drax::transport::pipeline::share_link(std::sync::Arc::new($l1), std::sync::Arc::new($l2));
    };
    ($l1:expr, $l2:expr, $($etc:expr)+) => {
        share_link!($l1, share_link!($l2, $($etc)+));
    };
}
