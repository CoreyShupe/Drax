pub trait PipelineStage {
    type Output<'a>;

    fn execute<'a>(&self) -> Self::Output<'a>;
}

pub trait Mapper {
    type Input<'a>;
    type Output<'a>;

    fn map<'a>(&self, input: Self::Input<'a>) -> Self::Output<'a>;
}

pub struct Map<
    I,
    O,
    IProvider: for<'a> PipelineStage<Output<'a> = I>,
    OMapper: for<'a> Mapper<Input<'a> = I, Output<'a> = O>,
> {
    input_provider: IProvider,
    output_mapper: OMapper,
}

impl<I, O, IProvider, OMapper> PipelineStage for Map<I, O, IProvider, OMapper>
where
    IProvider: for<'a> PipelineStage<Output<'a> = I>,
    OMapper: for<'a> Mapper<Input<'a> = I, Output<'a> = O>,
{
    type Output<'a> = O;

    fn execute<'a>(&self) -> Self::Output<'a> {
        let input = self.input_provider.execute();
        self.output_mapper.map(input)
    }
}
