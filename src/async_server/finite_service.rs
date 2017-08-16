use tokio_service::Service;

pub trait FiniteService: Service {
    fn has_finished(&self) -> Result<bool, <Self as Service>::Error>;
}
