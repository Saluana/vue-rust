use vue_rust_demo::app::App;
#[test]fn initial_quote(){let a=App::default();assert_eq!(a.labour(),600.0);assert_eq!(a.totalText(),"750.00");}
#[test]fn presets(){let mut a=App::default();a.mowingRate();assert_eq!(a.total(),630.0);a.cleanupRate();assert_eq!(a.total(),810.0);a.hedgeRate();assert_eq!(a.total(),750.0);}
#[test]fn fractional_hours_and_extras(){let mut a=App::default();a.hours=2.5;a.rate=90.0;a.disposal=75.0;a.materials=25.0;assert_eq!(a.totalText(),"325.00");a.clearExtras();assert_eq!(a.totalText(),"225.00");}
#[test]fn independent_quotes(){let mut a=App::default();a.clearExtras();assert_eq!(App::default().total(),750.0);}
