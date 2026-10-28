#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameStage {
    StartFrame,
    WorldPreFrame,
    AnimationPhaseOne,
    AnimationPhaseTwo,
    WorldPrepare,
    AnimationPhaseThree,
    WorldPrecursor,
    ActorPhysicsPublication,
    PostZeroJobs,
    Input,
    WorldAfterInput,
    PostInput,
    Logic,
    PreState,
    State,
    PostState,
    CollectSolverInputs,
    PrepareSolverJobs,
    SubmitSolver,
    WaitSolver,
    Adjust,
    PostAdjust,
    Output,
    PostOutput,
    FinishOutputs,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SchedulerState {
    Ready,
    Running,
    SolverPending,
    Faulted,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScheduleError<E> {
    WrongState {
        expected: SchedulerState,
        actual: SchedulerState,
    },
    Runtime {
        stage: FrameStage,
        source: E,
    },
}

pub trait AnimationPhases {
    type Error;

    fn phase_one(&mut self) -> Result<(), Self::Error>;
    fn phase_two(&mut self) -> Result<(), Self::Error>;
    fn phase_three(&mut self) -> Result<(), Self::Error>;
    fn publish_physics_input(&mut self) -> Result<(), Self::Error>;
}

pub trait WorldPhases {
    type Error;

    fn start_frame(&mut self, timestep: f32) -> Result<(), Self::Error>;
    fn pre_frame(&mut self) -> Result<(), Self::Error>;
    fn prepare(&mut self) -> Result<(), Self::Error>;
    fn precursor(&mut self) -> Result<(), Self::Error>;
    fn after_input(&mut self) -> Result<(), Self::Error>;
    fn collect_solver_inputs(&mut self) -> Result<(), Self::Error>;
    fn finish_outputs(&mut self) -> Result<(), Self::Error>;
}

pub trait PhysicalPhases {
    type Error;

    fn post_zero_jobs(&mut self) -> Result<(), Self::Error>;
    fn input(&mut self) -> Result<(), Self::Error>;
    fn post_input(&mut self) -> Result<(), Self::Error>;
    fn logic(&mut self) -> Result<(), Self::Error>;
    fn pre_state(&mut self) -> Result<(), Self::Error>;
    fn state(&mut self) -> Result<(), Self::Error>;
    fn post_state(&mut self) -> Result<(), Self::Error>;
    fn adjust(&mut self) -> Result<(), Self::Error>;
    fn post_adjust(&mut self) -> Result<(), Self::Error>;
    fn output(&mut self) -> Result<(), Self::Error>;
    fn post_output(&mut self) -> Result<(), Self::Error>;
}

pub trait SolverPhases {
    type Error;

    fn prepare_jobs(&mut self) -> Result<(), Self::Error>;
    fn submit(&mut self) -> Result<(), Self::Error>;
    fn wait(&mut self) -> Result<(), Self::Error>;
}

#[derive(Clone, Copy, Debug)]
pub struct FrameScheduler {
    state: SchedulerState,
}

impl Default for FrameScheduler {
    fn default() -> Self {
        Self {
            state: SchedulerState::Ready,
        }
    }
}

impl FrameScheduler {
    pub const fn state(&self) -> SchedulerState {
        self.state
    }

    pub fn run_before_solver<A, W, P, S, E>(
        &mut self,
        timestep: f32,
        animation: &mut A,
        world: &mut W,
        physical: &mut P,
        solver: &mut S,
    ) -> Result<(), ScheduleError<E>>
    where
        A: AnimationPhases<Error = E>,
        W: WorldPhases<Error = E>,
        P: PhysicalPhases<Error = E>,
        S: SolverPhases<Error = E>,
    {
        self.require(SchedulerState::Ready)?;
        self.state = SchedulerState::Running;

        self.call(FrameStage::StartFrame, world.start_frame(timestep))?;
        self.call(FrameStage::WorldPreFrame, world.pre_frame())?;
        self.call(FrameStage::AnimationPhaseOne, animation.phase_one())?;
        self.call(FrameStage::AnimationPhaseTwo, animation.phase_two())?;
        self.call(FrameStage::WorldPrepare, world.prepare())?;
        self.call(FrameStage::AnimationPhaseThree, animation.phase_three())?;
        self.call(FrameStage::WorldPrecursor, world.precursor())?;
        self.call(
            FrameStage::ActorPhysicsPublication,
            animation.publish_physics_input(),
        )?;
        self.call(FrameStage::PostZeroJobs, physical.post_zero_jobs())?;
        self.call(FrameStage::Input, physical.input())?;
        self.call(FrameStage::WorldAfterInput, world.after_input())?;
        self.call(FrameStage::PostInput, physical.post_input())?;
        self.call(FrameStage::Logic, physical.logic())?;
        self.call(FrameStage::PreState, physical.pre_state())?;
        self.call(FrameStage::State, physical.state())?;
        self.call(FrameStage::PostState, physical.post_state())?;
        self.call(
            FrameStage::CollectSolverInputs,
            world.collect_solver_inputs(),
        )?;
        self.call(FrameStage::PrepareSolverJobs, solver.prepare_jobs())?;
        self.call(FrameStage::SubmitSolver, solver.submit())?;
        self.state = SchedulerState::SolverPending;
        Ok(())
    }

    pub fn run_after_solver<W, P, S, E>(
        &mut self,
        world: &mut W,
        physical: &mut P,
        solver: &mut S,
    ) -> Result<(), ScheduleError<E>>
    where
        W: WorldPhases<Error = E>,
        P: PhysicalPhases<Error = E>,
        S: SolverPhases<Error = E>,
    {
        self.require(SchedulerState::SolverPending)?;
        self.state = SchedulerState::Running;

        self.call(FrameStage::WaitSolver, solver.wait())?;
        self.call(FrameStage::Adjust, physical.adjust())?;
        self.call(FrameStage::PostAdjust, physical.post_adjust())?;
        self.call(FrameStage::Output, physical.output())?;
        self.call(FrameStage::PostOutput, physical.post_output())?;
        self.call(FrameStage::FinishOutputs, world.finish_outputs())?;
        self.state = SchedulerState::Ready;
        Ok(())
    }

    fn require<E>(&self, expected: SchedulerState) -> Result<(), ScheduleError<E>> {
        if self.state == expected {
            Ok(())
        } else {
            Err(ScheduleError::WrongState {
                expected,
                actual: self.state,
            })
        }
    }

    fn call<E>(
        &mut self,
        stage: FrameStage,
        result: Result<(), E>,
    ) -> Result<(), ScheduleError<E>> {
        result.map_err(|source| {
            self.state = SchedulerState::Faulted;
            ScheduleError::Runtime { stage, source }
        })
    }
}

